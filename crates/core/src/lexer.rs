use crate::errors::{LexerError, LexerErrorReason};
use crate::types::{LexerSpan};
use crate::token::{Token, TokenKind};
use std::borrow::Cow;

#[rustfmt::skip]
struct Lexer<'a> {
    input:              &'a str,
    start:              usize,
    current:            usize,
    line:               usize,
    tokens:             Vec<Token<'a>>,

    // optimization
    last_size_memo:     Vec<usize>,
}

impl<'a> Lexer<'a> {
    #[rustfmt::skip]
    fn new(input: &'a str) -> Self {
        Self { input, start: 0, line: 1, current: 0, tokens: Vec::new(), last_size_memo: Vec::new() }
    }

    fn is_newline(&self, cmp: char) -> bool {
        matches!(cmp, '\n' | '\r' | '\u{000C}')
    }

    fn at_end(&self) -> bool {
        let x = self.input[self.current..].chars().next();
        if x.is_some() {
            return false;
        }

        true
    }

    fn at_ident_start(&self) -> bool {
        use utils::{ is_ident_start };

        let a = self.peek();
        let b = self.peek_n(1);

        match a {
            Some(c) if is_ident_start(c) => true,

            Some('-') => {
                match b {
                    Some('-') => true,
                    Some(c) if is_ident_start(c) => true,
                    Some('\\') => {
                        let c = self.peek_n(2);
                        matches!(c, Some(x) if !self.is_newline(x))
                    }
                    _ => false,
                }
            }

            Some('\\') => {
                matches!(b, Some(x) if !self.is_newline(x))
            }

            _ => false,
        }
    }

    fn at_valid_escape(&self) -> bool {
        if self.peek() != Some('\\') {
            return false;
        }

        match self.peek_next('\\') {
            Some(c) => !self.is_newline(c),
            None => false,
        }
    }

    pub fn scan(&mut self) -> &[Token<'a>] {
        match self.input.chars().next() {
            None => {
                self.add_token(Token::new(TokenKind::EOF, self.line, Cow::Borrowed("")));
                return &self.tokens[..];
            }
            Some(_) => {
                loop {
                    match self.run() {
                        Ok(()) => {}
                        Err(e) => {
                            // fatal lexer errors are reported, bad-token semantic is preserved
                        }
                    }
                    self.advance();
                    if self.at_end() {
                        break;
                    }
                }
            }
        }
        self.add_token(Token::new(TokenKind::EOF, self.line, Cow::Borrowed("")));

        &self.tokens[..]
    }

    #[rustfmt::skip]
    fn add_token(&mut self, token: Token<'a>) {
        self.tokens.push(token);
    }

    #[rustfmt::skip]
    fn run(&mut self) -> Result<(), LexerError> {
        // TODO: diagnostic consumers should only be used at the top level
        // they then can delegate to recognizing consumers
        self.start = self.current;
        // invariant: all advances in this function eventually stop at the end of a valid lexeme (or errors)
        match self.peek() {
            None =>  { return Err(LexerError::new(LexerErrorReason::INVARIANT_VIOLATION, self.line, LexerSpan (self.start, self.current))); },
            Some(cmp) => {
                match cmp {
                    x if utils::is_css_whitespace(x) => { self.whitespace(true)?; },
                    '"' | '\'' => { self.string(true)?; },
                    '{' => { self.add_token(Token::new(TokenKind::CURLY_OPEN, self.line, Cow::Borrowed("{"))); },
                    '}' => { self.add_token(Token::new(TokenKind::CURLY_CLOSE, self.line, Cow::Borrowed("}"))); },
                    '[' => { self.add_token(Token::new(TokenKind::BRACKET_OPEN, self.line, Cow::Borrowed("["))); },
                    ']' => { self.add_token(Token::new(TokenKind::BRACKET_CLOSE, self.line, Cow::Borrowed("]"))); },
                    '(' => { self.add_token(Token::new(TokenKind::PAREN_OPEN, self.line, Cow::Borrowed("("))); },
                    ')' => { self.add_token(Token::new(TokenKind::PAREN_CLOSE, self.line, Cow::Borrowed(")"))); },
                    ';' => { self.add_token(Token::new(TokenKind::SEMICOLON, self.line, Cow::Borrowed(";"))); },
                    ':' => { self.add_token(Token::new(TokenKind::COLON, self.line, Cow::Borrowed(":"))); },
                    ',' => { self.add_token(Token::new(TokenKind::COMMA, self.line, Cow::Borrowed(","))); },
                    '/' => {
                        if self.peek_next('/') == Some('*') {
                            self.comment()?;
                            return Ok(());
                        }
                        self.add_token(Token::new(TokenKind::SLASH, self.line, Cow::Borrowed("/")));
                    },
                    '+' | '.' => {
                        let delimiter = cmp;
                        match self.number(true) {
                            Ok(()) => return Ok(()),
                            Err(e) if e.reason == LexerErrorReason::MATCHED_PREFIX => {
                                self.add_token(Token::new(
                                    TokenKind::DELIM(delimiter),
                                    self.line,
                                    Cow::Borrowed(""),
                                ));
                                return Ok(());
                            }
                            Err(e) => return Err(e),
                        }
                    },
                    d if d.is_ascii_digit() => {
                        self.number(true)?;
                    },
                    '\\' => {
                        self.escape(true)?;
                    },
                    '@' => {
                        match self.at_keyword(true) {
                            Ok(()) => return Ok(()),
                            Err(e) if e.reason == LexerErrorReason::MATCHED_PREFIX => {
                                self.add_token(Token::new(
                                    TokenKind::DELIM('@'),
                                    self.line,
                                    Cow::Borrowed(""),
                                ));
                                return Ok(());
                            },
                            Err(e) => return Err(e),
                        }
                    },
                    identifier => {
                        if self.input[self.current..].starts_with("<!--") {
                            self.advance();
                            self.advance();
                            self.advance();
                            self.add_token(Token::new(TokenKind::CDO, self.line, Cow::Borrowed("<!--")));
                            return Ok(());
                        }

                        if self.input[self.current..].starts_with("-->") {
                            self.advance();
                            self.advance();
                            self.add_token(Token::new(TokenKind::CDC, self.line, Cow::Borrowed("-->")));
                            return Ok(());
                        }

                        if self.starts_url_function() {
                            self.advance();
                            self.advance();
                            self.advance();
                            return self.url(true);
                        }

                        if self.at_ident_start() {
                            match self.function(true) {
                                Ok(()) => return Ok(()),
                                Err(e) if matches!(
                                    e.reason,
                                    LexerErrorReason::NO_MATCH
                                        | LexerErrorReason::MATCHED_PREFIX
                                ) => {
                                    self.ident(true)?;
                                    return Ok(());
                                }
                                Err(e) => return Err(e),
                            }
                        }

                        if identifier == '-' {
                            match self.number(true) {
                                Ok(()) => return Ok(()),
                                Err(e) if e.reason == LexerErrorReason::MATCHED_PREFIX => {
                                    self.add_token(Token::new(
                                        TokenKind::DELIM('-'),
                                        self.line,
                                        Cow::Borrowed(""),
                                    ));
                                    return Ok(());
                                }
                                Err(e) => return Err(e),
                            }
                        }

                        if self.input[self.current..].starts_with("!important") {
                            self.current += 9;
                            self.add_token(Token::new(TokenKind::IMPORTANT_TOKEN, self.line, Cow::Borrowed("!important")));
                            return Ok(());
                        }

                        if identifier == '#' {
                            match self.hash(true) {
                                Ok(()) => { return Ok(()); },
                                Err(_) => {
                                    self.add_token(Token::new(TokenKind::DELIM('#'), self.line, Cow::Borrowed("")));
                                    return Ok(());
                                }
                            };
                        }

                        self.add_token(Token::new(
                            TokenKind::DELIM(identifier),
                            self.line,
                            Cow::Borrowed(""),
                        ));
                        return Ok(());

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
            // invariant assumption: advance will always be called before any setback-lookback combination
            // that way, the references to last size are valid at any given time
            self.last_size_memo.push(len);
        }
    }

    fn step_back(&mut self) {
        // stepback to the end of the last matched lexeme in the current consumer - preserving the invariant @ self.run
        if let Some(pop) = self.last_size_memo.pop() {
            self.current -= pop;
        }
    }

    fn peek(&self) -> Option<char> {
        self.input[self.current..].chars().next()
    }

    fn peek_next(&self, pop: char) -> Option<char> {
        self.input[self.current + pop.len_utf8()..].chars().next()
    }

    fn peek_next_next(&self, pop: char) -> Option<char> {
        if let Some(next) = self.input[self.current + pop.len_utf8()..].chars().next() {
            if let Some(next_next) = self.input[self.current + pop.len_utf8() + next.len_utf8()..].chars().next() {
                return Some(next_next);
            }
            return None;
        };

        None
    }

    fn peek_n(&self, n: usize) -> Option<char> {
        self.input[self.current..].chars().nth(n)
    }

    fn lookback(&mut self) -> Option<char> {
        if let Some(pop) = self.last_size_memo.pop() {
            return self.input[self.current - pop..].chars().next();
        }

        None
    }

    fn catch_match(&mut self, cmp: char) -> bool {
        if self.at_end() {
            return false;
        }
        // invariant: if not at end, there should be a next char
        let pop = self.input[self.current..].chars().next().unwrap();
        if pop != cmp {
            return false;
        }
        self.advance();

        true
    }

    fn backtrack(&mut self, branch_point: usize, memo_point: usize) {
        self.current = branch_point;
        self.last_size_memo.truncate(memo_point);
    }

    fn function(&mut self, collect: bool) -> Result<(), LexerError> {
        let branch_point = self.current;
        let memo_point = self.last_size_memo.len();

        if !self.at_ident_start() {
            return Err(LexerError::new(
                LexerErrorReason::NO_MATCH,
                self.line,
                LexerSpan(self.start, self.current),
            ));
        }

        self.ident(false)?;

        self.advance();
        match self.peek() {
            Some('(') => {
                self.advance();
            },
            _ => {
                self.backtrack(branch_point, memo_point);

                return Err(LexerError::new(
                    LexerErrorReason::MATCHED_PREFIX,
                    self.line,
                    LexerSpan (self.start, self.current),
                ));
            }
        };

        self.step_back();

        if collect {
            self.add_token(Token::new(
                TokenKind::FUNCTION,
                self.line,
                Cow::Borrowed(""),
            ));
        }

        Ok(())
    }

    fn at_keyword(&mut self, collect: bool) -> Result<(), LexerError> {
        if !self.catch_match('@') {
            return Err(LexerError::new(
                LexerErrorReason::NO_MATCH,
                self.line,
                LexerSpan(self.start, self.current),
            ));
        }

        if !self.at_ident_start() {
            self.step_back();
            return Err(LexerError::new(
                LexerErrorReason::MATCHED_PREFIX,
                self.line,
                LexerSpan(self.start, self.current),
            ));
        }

        self.ident(false)?;

        if collect {
            self.add_token(Token::new(
                TokenKind::AT_KEYWORD,
                self.line,
                Cow::Borrowed(""),
            ));
        }

        Ok(())
    }

    fn hash(&mut self, collect: bool) -> Result<(), LexerError> {
        use utils::{ is_ident };

        let mut should_align = false; // technically redundant here but i'll stick to convention for now
        let mut id_hash = false;

        if !self.catch_match('#') {
            return Err(LexerError::new(
                LexerErrorReason::NO_MATCH,
                self.line,
                LexerSpan (self.start, self.current),
            ));
        }

        should_align = true;

        if self.at_ident_start() {
            id_hash = true;
        }

        match self.peek() {
            Some(x) if is_ident(x) => {
                self.advance();
            },
            Some(y) if y == '\\' => {
                match self.escape(false) {
                    Ok(()) => {},
                    _ => {
                        self.step_back();

                        return Err(LexerError::new(LexerErrorReason::UNTERMINATED_TOKEN, self.line, LexerSpan (self.start, self.current)));
                    }
                }
            },
            Some(n) => {
                self.step_back();

                return Err(LexerError::new(LexerErrorReason::UNTERMINATED_TOKEN, self.line, LexerSpan (self.start, self.current)));
            },
            None => {
                self.step_back();

                return Err(LexerError::new(LexerErrorReason::UNTERMINATED_TOKEN, self.line, LexerSpan (self.start, self.current)));
            }
        }

        loop {
            if let Some(curr) = self.peek() {
                if is_ident(curr) {
                    self.advance();
                    should_align = true;
                } else if self.at_valid_escape() {
                    match self.escape(false) {
                        Ok(()) => {
                            self.advance();
                            should_align = true;
                            continue;
                        },
                        _ => {
                            if should_align {
                                self.step_back();

                                return Err(LexerError::new(LexerErrorReason::UNTERMINATED_TOKEN, self.line,  LexerSpan (self.start, self.current)));
                            }
                        }
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        if should_align {
            self.step_back();
        }

        if id_hash && collect {
            self.add_token(Token::new(TokenKind::ID_HASH, self.line, Cow::Borrowed("")));
        } else if !id_hash && collect {
            self.add_token(Token::new(TokenKind::GENERIC_HASH, self.line, Cow::Borrowed("")));
        }

        Ok(())
    }

    fn number(&mut self, collect: bool) -> Result<(), LexerError> {
        let branch_point = self.current;
        let memo_point = self.last_size_memo.len();

        if matches!(self.peek(), Some('+' | '-')) {
            self.advance();
        }

        let starts_fraction = self.peek() == Some('.')
            && matches!(self.peek_n(1), Some(c) if c.is_ascii_digit());

        if starts_fraction {
            self.advance();
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.advance();
            }
        } else if matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.advance();
            }

            if self.peek() == Some('.')
                && matches!(self.peek_n(1), Some(c) if c.is_ascii_digit())
            {
                self.advance();
                while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                    self.advance();
                }
            }
        } else {
            self.backtrack(branch_point, memo_point);
            return Err(LexerError::new(
                LexerErrorReason::MATCHED_PREFIX,
                self.line,
                LexerSpan(self.start, self.current),
            ));
        }

        let exponent = matches!(self.peek(), Some('e' | 'E'))
            && (matches!(self.peek_n(1), Some(c) if c.is_ascii_digit())
                || (matches!(self.peek_n(1), Some('+' | '-'))
                    && matches!(self.peek_n(2), Some(c) if c.is_ascii_digit())));

        if exponent {
            self.advance();
            if matches!(self.peek(), Some('+' | '-')) {
                self.advance();
            }
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.advance();
            }
        }

        let kind = if self.peek() == Some('%') {
            TokenKind::PERCENTAGE
        } else if self.at_ident_start() {
            self.ident(false)?;
            TokenKind::DIMENSION
        } else {
            self.step_back();
            TokenKind::NUMBER
        };

        if collect {
            self.add_token(Token::new(kind, self.line, Cow::Borrowed("")));
        }

        Ok(())
    }

    fn url(&mut self, collect: bool) -> Result<(), LexerError> {
        if self.peek() != Some('(') {
            return Err(LexerError::new(
                LexerErrorReason::NO_MATCH,
                self.line,
                LexerSpan (self.start, self.current),
            ));
        }

        self.advance();
        self.consume_url_whitespace();

        if matches!(self.peek(), Some('"' | '\'')) {
            return self.consume_bad_url(collect);
        }

        loop {
            match self.peek() {
                None => {
                    self.step_back();
                    if collect {
                        self.add_token(Token::new(TokenKind::URL, self.line, Cow::Borrowed("")));
                    }
                    return Ok(());
                }
                Some(')') => {
                    if collect {
                        self.add_token(Token::new(TokenKind::URL, self.line, Cow::Borrowed("")));
                    }
                    return Ok(());
                }
                Some(c) if utils::is_css_whitespace(c) => {
                    self.consume_url_whitespace();
                    if self.peek() == Some(')') || self.at_end() {
                        if self.at_end() {
                            self.step_back();
                        }
                        if collect {
                            self.add_token(Token::new(TokenKind::URL, self.line, Cow::Borrowed("")));
                        }
                        return Ok(());
                    }
                    return self.consume_bad_url(collect);
                }
                Some('"' | '\'' | '(') => return self.consume_bad_url(collect),
                Some('\\') => {
                    if !self.at_valid_escape() {
                        return self.consume_bad_url(collect);
                    }
                    self.escape(false)?;
                    self.advance();
                }
                Some(c) if !utils::is_css_printable(c) => {
                    return self.consume_bad_url(collect);
                }
                Some(_) => self.advance(),
            }
        }
    }

    fn starts_url_function(&self) -> bool {
        matches!(self.peek_n(0), Some('u' | 'U'))
            && matches!(self.peek_n(1), Some('r' | 'R'))
            && matches!(self.peek_n(2), Some('l' | 'L'))
            && self.peek_n(3) == Some('(')
    }

    fn consume_url_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if !utils::is_css_whitespace(c) {
                break;
            }
            if self.is_newline(c) {
                self.line += 1;
            }
            self.advance();
        }
    }

    fn consume_bad_url(&mut self, collect: bool) -> Result<(), LexerError> {
        while let Some(c) = self.peek() {
            if c == ')' {
                if collect {
                    self.add_token(Token::new(TokenKind::BAD_URL, self.line, Cow::Borrowed("")));
                }
                return Ok(());
            }
            if self.is_newline(c) {
                self.line += 1;
            }
            self.advance();
        }

        self.step_back();
        if collect {
            self.add_token(Token::new(TokenKind::BAD_URL, self.line, Cow::Borrowed("")));
        }
        Ok(())
    }

    fn ident(&mut self, collect: bool) -> Result<(), LexerError> {
        use utils::{is_ident, is_ident_start};

        let mut should_align = false;

        if !self.at_ident_start() {
            return Err(LexerError::new(
                LexerErrorReason::NO_MATCH,
                self.line,
                LexerSpan(self.start, self.current),
            ));
        }

        match self.peek() {
            Some(x) if x == '-' => {
                self.advance();
                should_align = true;

                if let Some(curr) = self.peek() {
                    if curr == '-' {
                        self.advance();
                    } else if is_ident_start(curr) {
                        self.advance();
                    } else if curr == '\\' {
                        if !self.at_valid_escape() {
                            self.step_back();
                            return Err(LexerError::new(
                                LexerErrorReason::UNTERMINATED_TOKEN,
                                self.line,
                                LexerSpan (self.start, self.current),
                            ));
                        }

                        match self.escape(false) {
                            Ok(_) => {}
                            Err(e) if e.reason == LexerErrorReason::NO_MATCH => {}
                            _ => {
                                // TODO: handle cursor reset
                            }
                        }
                    } else {
                        self.step_back();
                        return Err(LexerError::new(
                            LexerErrorReason::UNTERMINATED_TOKEN,
                            self.line,
                            LexerSpan (self.start, self.current),
                        ));
                    }
                } else {
                    self.step_back();
                    return Err(LexerError::new(
                        LexerErrorReason::UNTERMINATED_TOKEN,
                        self.line,
                        LexerSpan (self.start, self.current),
                    ));
                }
            }
            Some(u) if is_ident_start(u) => {
                self.advance();
                should_align = true;
            }
            Some(v) if v == '\\' => {
                if !self.at_valid_escape() {
                    if should_align {
                        self.step_back();
                    }
                    return Err(LexerError::new(
                        LexerErrorReason::UNTERMINATED_TOKEN,
                        self.line,
                        LexerSpan (self.start, self.current),
                    ));
                }

                match self.escape(false) {
                    Ok(_) => {
                        self.advance();
                        should_align = true;
                    }
                    Err(e) if e.reason == LexerErrorReason::NO_MATCH => {}
                    _ => {
                        // TODO: handle cursor reset
                    }
                }
            }
            _ => {
                if should_align {
                    self.step_back();
                }
                return Err(LexerError::new(
                    LexerErrorReason::UNTERMINATED_TOKEN,
                    self.line,
                    LexerSpan (self.start, self.current),
                ));
            }
        };

        loop {
            match self.peek() {
                Some(x) if is_ident(x) => {
                    self.advance();
                    should_align = true;
                }
                Some(v) if v == '\\' => {
                    if !self.at_valid_escape() {
                        if should_align {
                            self.step_back();
                        }
                        return Err(LexerError::new(
                            LexerErrorReason::UNTERMINATED_TOKEN,
                            self.line,
                            LexerSpan (self.start, self.current),
                        ));
                    }

                    match self.escape(false) {
                        Ok(_) => {
                            self.advance();
                            should_align = true;
                        }
                        Err(e) if e.reason == LexerErrorReason::NO_MATCH => {}
                        _ => {
                            // TODO: handle cursor reset
                        }
                    }
                }
                _ => {
                    if should_align {
                        self.step_back();
                    }
                    break;
                }
            }
        }

        if collect {
            self.add_token(Token::new(TokenKind::IDENT, self.line, Cow::Borrowed("")));
        }

        Ok(())
    }

    fn string(&mut self, collect: bool) -> Result<(), LexerError> {
        if self.at_end() {
            return Err(LexerError::new(
                LexerErrorReason::NO_MATCH,
                self.line,
                LexerSpan (self.start, self.current),
            ));
        }
        // invariant: if not at end, there should be a next char
        let terminal = self.peek().unwrap();
        if terminal != '"' && terminal != '\'' {
            return Err(LexerError::new(
                LexerErrorReason::NO_MATCH,
                self.line,
                LexerSpan (self.start, self.current),
            ));
        }

        self.advance();

        loop {
            if self.at_end() {
                self.step_back();
                if collect {
                    self.add_token(Token::new(
                        TokenKind::BAD_STRING,
                        self.line,
                        Cow::Borrowed(""),
                    ));
                }
                return Ok(());
            }

            match self.peek() {
                Some('\\') => {
                    let mut backslash_count = 0;
                    while self.peek() == Some('\\') {
                        self.advance();
                        backslash_count += 1;
                    }

                    // An odd-length run escapes the code point following it;
                    // an even-length run leaves that code point available to be interpreted normally on the next iteration.
                    if backslash_count % 2 == 1 {
                        if self.at_end() {
                            self.step_back();
                            if collect {
                                self.add_token(Token::new(
                                    TokenKind::BAD_STRING,
                                    self.line,
                                    Cow::Borrowed(""),
                                ));
                            }
                            return Ok(());
                        }

                        if self.peek() == Some('\r') && self.peek_next('\r') == Some('\n') {
                            self.advance();
                        }
                        self.advance();
                    }
                }
                Some(x) if x == terminal => {
                    if collect {
                        self.add_token(Token::new(
                            TokenKind::STRING,
                            self.line,
                            Cow::Borrowed(""),
                        ));
                    }
                    return Ok(());
                }
                Some(x) if self.is_newline(x) => {
                    self.step_back();
                    if collect {
                        self.add_token(Token::new(
                            TokenKind::BAD_STRING,
                            self.line,
                            Cow::Borrowed(""),
                        ));
                    }
                    return Ok(());
                }
                Some(x) if self.is_newline(x) => {
                    self.advance();
                }
                Some(_) => self.advance(),
                None => unreachable!("at_end was checked above"),
            }
        }
    }

    fn escape(&mut self, collect: bool) -> Result<(), LexerError> {
        let mut should_align = false;

        if self.peek() != Some('\\') {
            return Err(LexerError::new(
                LexerErrorReason::NO_MATCH,
                self.line,
                LexerSpan (self.start, self.current),
            ));
        }

        self.advance();
        should_align = true;
        match self.peek() {
            Some(x) if x.is_ascii_digit() => {
                self.hex_token(false);
                if collect {
                    self.add_token(Token::new(TokenKind::ESCAPE, self.line, Cow::Borrowed("")));
                }

                Ok(())
            }
            Some(n) if self.is_newline(n) => {
                self.step_back();

                Err(LexerError::new(
                    LexerErrorReason::INVALID_TOKEN,
                    self.line,
                    LexerSpan (self.start, self.current),
                ))
            }
            Some(_) => {
                if collect {
                    self.add_token(Token::new(TokenKind::ESCAPE, self.line, Cow::Borrowed("")));
                }

                Ok(())
            }
            None => {
                self.step_back();

                Err(LexerError::new(
                    LexerErrorReason::INVALID_TOKEN,
                    self.line,
                    LexerSpan (self.start, self.current),
                ))
            }
        }
    }

    fn hex_token(&mut self, collect: bool) -> Result<(), LexerError> {
        let mut count = 0;
        let mut should_align = false;

        while count < 6 {
            match self.peek() {
                Some(c) if c.is_ascii_hexdigit() => {
                    self.advance();
                    count += 1;
                    should_align = true;
                }

                _ => break,
            }
        }

        if let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.advance();
                should_align = true;
            }
        }

        if should_align {
            self.step_back();
        }

        if collect {
            self.add_token(Token::new(
                TokenKind::HEX_TOKEN,
                self.line,
                Cow::Borrowed(""),
            ));
        }

        Ok(())
    }

    fn comment(&mut self) -> Result<(), LexerError> {
        loop {
            if self.at_end() {
                return Err(LexerError::new(
                    LexerErrorReason::UNTERMINATED_TOKEN,
                    self.line,
                    LexerSpan (self.start, self.current),
                ));
            }
            // invariant: if not at end, there should be a next char
            let x = self.peek().unwrap();

            if x == '*' {
                if let Some(y) = self.peek_next(x) {
                    if y == '/' {
                        self.advance();
                        return Ok(());
                    }

                    self.advance();
                    continue;
                }
                return Err(LexerError::new(
                    LexerErrorReason::UNTERMINATED_TOKEN,
                    self.line,
                    LexerSpan (self.start, self.current),
                ));
            } else if self.is_newline(x) {
                self.line += 1;
            }

            self.advance();
        }
    }

    fn digit(&mut self, collect: bool) -> Result<(), LexerError> {
        let mut should_align = false;

        while let Some(di) = self.peek() {
            if !di.is_ascii_digit() {
                break;
            }
            should_align = true;
            self.advance();
        }

        if should_align {
            self.step_back();
            if collect {
                self.add_token(Token::new(
                    TokenKind::DIGIT_TOKEN,
                    self.line,
                    Cow::Borrowed(""),
                ));
            }
            return Ok(());
        }

        Ok(())
    }

    fn whitespace(&mut self, collect: bool) -> Result<(), LexerError> {
        let mut should_align = false;

        while let Some(x) = self.peek() {
            if !utils::is_css_whitespace(x) {
                break;
            }
            should_align = true;

            if x == '\r' && self.peek_next('\r') == Some('\n') {
                self.advance();
                self.advance();
                self.line += 1;
            } else {
                if self.is_newline(x) {
                    self.line += 1;
                }
                self.advance();
            }
        }

        if should_align {
            self.step_back();
            if collect {
                self.add_token(Token::new(
                    TokenKind::WHITESPACE,
                    self.line,
                    Cow::Borrowed(""),
                ));
            }
            return Ok(());
        }

        Err(LexerError::new(
            LexerErrorReason::NO_MATCH,
            self.line,
            LexerSpan (self.start, self.current),
        ))
    }
}

mod utils {
    pub fn is_css_whitespace(c: char) -> bool {
        matches!(c, '\u{0009}' | '\u{000A}' | '\u{000C}' | '\u{000D}' | '\u{0020}')
    }

    pub fn is_css_printable(c: char) -> bool {
        !matches!(
            c,
            '\u{0000}'..='\u{0008}'
                | '\u{000B}'
                | '\u{000E}'..='\u{001F}'
                | '\u{007F}'..='\u{009F}'
        )
    }

    pub fn is_ident_start(c: char) -> bool {
        c == '_' || c.is_ascii_alphabetic() || !c.is_ascii()
    }

    pub fn is_ident(c: char) -> bool {
        is_ident_start(c) || c.is_ascii_digit() || c == '-'
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advance_moves_by_utf8_code_point_width() {
        let mut lexer = Lexer::new("aé🙂");

        assert_eq!(lexer.current, 0);
        lexer.advance();
        assert_eq!(lexer.current, 1);
        lexer.advance();
        assert_eq!(lexer.current, 3);
        lexer.advance();
        assert_eq!(lexer.current, 7);
        assert!(lexer.at_end());
    }

    #[test]
    fn step_back_restores_the_last_advanced_code_point() {
        let mut lexer = Lexer::new("éx");

        lexer.advance();
        assert_eq!(lexer.current, 2);
        assert_eq!(lexer.last_size_memo, vec![2]);

        lexer.step_back();
        assert_eq!(lexer.current, 0);
        assert!(lexer.last_size_memo.is_empty());
    }

    #[test]
    fn valid_escape_requires_a_non_newline_following_code_point() {
        assert!(Lexer::new("\\a").at_valid_escape());
        assert!(!Lexer::new("\\\n").at_valid_escape());
        assert!(!Lexer::new("\\").at_valid_escape());
    }

    #[test]
    fn whitespace_consumer_stops_on_the_last_whitespace_code_point() {
        let mut lexer = Lexer::new(" \tA");

        assert!(lexer.whitespace(false).is_ok());
        assert_eq!(lexer.peek(), Some('\t'));
        lexer.advance();
        assert_eq!(lexer.peek(), Some('A'));
    }

    #[test]
    fn whitespace_consumer_accepts_only_css_whitespace_code_points() {
        let mut lexer = Lexer::new("\t\n\u{000C}\r A");

        assert!(lexer.whitespace(true).is_ok());
        assert_eq!(lexer.tokens[0].kind, TokenKind::WHITESPACE);
        assert_eq!(lexer.peek(), Some(' '));
        assert_eq!(lexer.line, 4);

        lexer.advance();
        assert_eq!(lexer.peek(), Some('A'));
    }

    #[test]
    fn whitespace_consumer_treats_crlf_as_one_newline() {
        let mut lexer = Lexer::new(" \r\nX");

        assert!(lexer.whitespace(false).is_ok());
        assert_eq!(lexer.peek(), Some('\n'));
        assert_eq!(lexer.line, 2);

        lexer.advance();
        assert_eq!(lexer.peek(), Some('X'));
    }

    #[test]
    fn non_breaking_space_is_not_whitespace_and_starts_an_ident() {
        let mut lexer = Lexer::new("\u{00A0}x");

        assert!(lexer.run().is_ok());
        assert_eq!(lexer.tokens[0].kind, TokenKind::IDENT);
        assert_eq!(lexer.peek(), Some('x'));

        lexer.advance();
        assert!(lexer.at_end());
    }

    #[test]
    fn digit_consumer_stops_on_the_last_digit() {
        let mut lexer = Lexer::new("123x");

        assert!(lexer.digit(false).is_ok());
        assert_eq!(lexer.peek(), Some('3'));
        lexer.advance();
        assert_eq!(lexer.peek(), Some('x'));
    }

    #[test]
    fn number_consumer_classifies_percentage_and_preserves_cursor() {
        let mut lexer = Lexer::new("10%x");

        assert!(lexer.number(true).is_ok());
        assert_eq!(lexer.peek(), Some('%'));
        assert_eq!(lexer.tokens[0].kind, TokenKind::PERCENTAGE);

        lexer.advance();
        assert_eq!(lexer.peek(), Some('x'));
    }

    #[test]
    fn number_consumer_classifies_dimension_and_preserves_cursor() {
        let mut lexer = Lexer::new("1.5rem;");

        assert!(lexer.number(true).is_ok());
        assert_eq!(lexer.peek(), Some('m'));
        assert_eq!(lexer.tokens[0].kind, TokenKind::DIMENSION);

        lexer.advance();
        assert_eq!(lexer.peek(), Some(';'));
    }

    #[test]
    fn number_consumer_restores_state_for_a_non_number_prefix() {
        let mut lexer = Lexer::new("+");

        let result = lexer.number(false);
        assert!(matches!(
            result,
            Err(LexerError {
                reason: LexerErrorReason::MATCHED_PREFIX,
                ..
            })
        ));
        assert_eq!(lexer.current, 0);
        assert!(lexer.last_size_memo.is_empty());
    }

    #[test]
    fn number_consumer_handles_css_number_forms() {
        let cases = [
            ("0", TokenKind::NUMBER, '0'),
            ("+1", TokenKind::NUMBER, '1'),
            ("-1", TokenKind::NUMBER, '1'),
            (".5", TokenKind::NUMBER, '5'),
            ("+.5", TokenKind::NUMBER, '5'),
            ("-.5", TokenKind::NUMBER, '5'),
            ("1.25", TokenKind::NUMBER, '5'),
            ("1e3", TokenKind::NUMBER, '3'),
            ("1E+3", TokenKind::NUMBER, '3'),
            ("1e-3", TokenKind::NUMBER, '3'),
            ("10%", TokenKind::PERCENTAGE, '%'),
            ("2px", TokenKind::DIMENSION, 'x'),
            ("1e3ms", TokenKind::DIMENSION, 's'),
            ("1é", TokenKind::DIMENSION, 'é'),
            ("1\\70 x", TokenKind::DIMENSION, 'x'),
        ];

        for (input, expected_kind, expected_last) in cases {
            let mut lexer = Lexer::new(input);

            assert!(lexer.number(true).is_ok(), "input: {input}");
            assert_eq!(lexer.tokens[0].kind, expected_kind, "input: {input}");
            assert_eq!(lexer.peek(), Some(expected_last), "input: {input}");
        }
    }

    #[test]
    fn number_consumer_leaves_non_numeric_suffixes_for_later_tokens() {
        let cases: [(&str, char, Option<char>); 7] = [
            ("1.", '1', Some('.')),
            ("1.2.3", '2', Some('.')),
            ("1e", 'e', None),
            ("1e+", 'e', Some('+')),
            ("1e-", '-', None),
            ("1e+foo", 'e', Some('+')),
            ("10%%", '%', Some('%')),
        ];

        for (input, expected_last, expected_next) in cases {
            let mut lexer = Lexer::new(input);

            assert!(lexer.number(true).is_ok(), "input: {input}");
            assert_eq!(lexer.peek(), Some(expected_last), "input: {input}");

            lexer.advance();
            assert_eq!(lexer.peek(), expected_next, "input: {input}");
        }
    }

    #[test]
    fn plain_number_aligns_before_a_following_delimiter() {
        let mut lexer = Lexer::new("42)");

        assert!(lexer.number(true).is_ok());
        assert_eq!(lexer.tokens[0].kind, TokenKind::NUMBER);
        assert_eq!(lexer.peek(), Some('2'));

        lexer.advance();
        assert_eq!(lexer.peek(), Some(')'));
    }

    #[test]
    fn string_consumer_handles_closed_strings() {
        let mut lexer = Lexer::new("\"hello\"x");

        assert!(lexer.string(true).is_ok());
        assert_eq!(lexer.tokens[0].kind, TokenKind::STRING);
        assert_eq!(lexer.peek(), Some('"'));

        lexer.advance();
        assert_eq!(lexer.peek(), Some('x'));
    }

    #[test]
    fn string_consumer_emits_bad_string_at_eof() {
        let mut lexer = Lexer::new("\"unterminated");

        assert!(lexer.string(true).is_ok());
        assert_eq!(lexer.tokens[0].kind, TokenKind::BAD_STRING);
        assert_eq!(lexer.peek(), Some('d'));

        lexer.advance();
        assert!(lexer.at_end());
    }

    #[test]
    fn string_consumer_emits_bad_string_before_unescaped_newlines() {
        for newline in ['\n', '\r', '\u{000C}'] {
            let input = format!("\"before{newline}after");
            let mut lexer = Lexer::new(&input);

            assert!(lexer.string(true).is_ok());
            assert_eq!(lexer.tokens[0].kind, TokenKind::BAD_STRING);
            assert_eq!(lexer.peek(), Some('e'));

            lexer.advance();
            assert_eq!(lexer.peek(), Some(newline));
        }
    }

    #[test]
    fn string_consumer_accepts_escaped_newlines() {
        let mut lexer = Lexer::new("\"before\\\nafter\"");

        assert!(lexer.string(true).is_ok());
        assert_eq!(lexer.tokens[0].kind, TokenKind::STRING);
        assert_eq!(lexer.peek(), Some('"'));
    }

    #[test]
    fn string_consumer_uses_backslash_parity_for_quotes() {
        let mut escaped = Lexer::new("\"a\\\"b\"");
        assert!(escaped.string(true).is_ok());
        assert_eq!(escaped.tokens[0].kind, TokenKind::STRING);
        assert_eq!(escaped.peek(), Some('"'));

        let mut unescaped = Lexer::new("\"a\\\\\"x");
        assert!(unescaped.string(true).is_ok());
        assert_eq!(unescaped.tokens[0].kind, TokenKind::STRING);
        assert_eq!(unescaped.peek(), Some('"'));
    }

    #[test]
    fn url_consumer_accepts_printable_content_and_trailing_whitespace() {
        let mut lexer = Lexer::new("url(foo   )x");

        assert!(lexer.run().is_ok());
        assert_eq!(lexer.tokens[0].kind, TokenKind::URL);
        assert_eq!(lexer.peek(), Some(')'));

        lexer.advance();
        assert_eq!(lexer.peek(), Some('x'));
    }

    #[test]
    fn url_consumer_accepts_empty_urls_and_valid_escapes() {
        let mut empty = Lexer::new("url( )");
        assert!(empty.run().is_ok());
        assert_eq!(empty.tokens[0].kind, TokenKind::URL);
        assert_eq!(empty.peek(), Some(')'));

        let mut escaped = Lexer::new("url(foo\\ bar)");
        assert!(escaped.run().is_ok());
        assert_eq!(escaped.tokens[0].kind, TokenKind::URL);
        assert_eq!(escaped.peek(), Some(')'));
    }

    #[test]
    fn url_consumer_emits_bad_url_for_invalid_contents() {
        for input in [
            "url(\"foo\")",
            "url(foo(bar))",
            "url(foo\nbar)",
            "url(foo\u{0000}bar)",
            "url(foo bar baz)",
            "url(foo\\",
        ] {
            let mut lexer = Lexer::new(input);

            assert!(lexer.run().is_ok(), "input: {input:?}");
            assert_eq!(lexer.tokens[0].kind, TokenKind::BAD_URL, "input: {input:?}");
        }
    }

    #[test]
    fn url_consumer_returns_url_at_eof_after_valid_unquoted_content() {
        for input in ["url(foo", "url(foo   ", "url(   "] {
            let mut lexer = Lexer::new(input);

            assert!(lexer.run().is_ok(), "input: {input:?}");
            assert_eq!(lexer.tokens[0].kind, TokenKind::URL, "input: {input:?}");
            assert!(lexer.at_end() || lexer.peek().is_some(), "input: {input:?}");
        }
    }

    #[test]
    fn url_consumer_accepts_hex_and_non_ascii_escapes() {
        for input in ["url(\\70 x)", "url(é)", "url(\\é)"] {
            let mut lexer = Lexer::new(input);

            assert!(lexer.run().is_ok(), "input: {input:?}");
            assert_eq!(lexer.tokens[0].kind, TokenKind::URL, "input: {input:?}");
        }
    }

    #[test]
    fn bad_url_consumer_consumes_remnants_through_the_closing_parenthesis() {
        let mut lexer = Lexer::new("url(\"bad\")next");

        assert!(lexer.run().is_ok());
        assert_eq!(lexer.tokens[0].kind, TokenKind::BAD_URL);
        assert_eq!(lexer.peek(), Some(')'));

        lexer.advance();
        assert_eq!(lexer.peek(), Some('n'));
    }

    #[test]
    fn url_matching_is_ascii_case_insensitive_but_does_not_match_prefixes() {
        let mut uppercase = Lexer::new("URL(foo)");
        assert!(uppercase.run().is_ok());
        assert_eq!(uppercase.tokens[0].kind, TokenKind::URL);

        let mut prefix = Lexer::new("urlx(foo)");
        assert!(prefix.run().is_ok());
        assert_eq!(prefix.tokens[0].kind, TokenKind::FUNCTION);
    }

    #[test]
    fn run_emits_delim_for_unmatched_code_points() {
        for delimiter in ['*', '=', '>', '<', '~', '|', '^', '$', '&', '!', '?', '%'] {
            let input = format!("{delimiter}x");
            let mut lexer = Lexer::new(&input);

            assert!(lexer.run().is_ok(), "delimiter: {delimiter}");
            assert_eq!(lexer.tokens[0].kind, TokenKind::DELIM(delimiter));
            assert_eq!(lexer.peek(), Some(delimiter));

            lexer.advance();
            assert_eq!(lexer.peek(), Some('x'));
        }
    }

    #[test]
    fn run_emits_one_delim_for_a_non_comment_slash() {
        let mut lexer = Lexer::new("/x");

        assert!(lexer.run().is_ok());
        assert_eq!(lexer.tokens.len(), 1);
        assert_eq!(lexer.tokens[0].kind, TokenKind::SLASH);
        assert_eq!(lexer.peek(), Some('/'));
    }

    #[test]
    fn run_emits_cdo_and_cdc_with_correct_cursor_alignment() {
        let mut cdo = Lexer::new("<!--x");
        assert!(cdo.run().is_ok());
        assert_eq!(cdo.tokens[0].kind, TokenKind::CDO);
        assert_eq!(cdo.peek(), Some('-'));
        cdo.advance();
        assert_eq!(cdo.peek(), Some('x'));

        let mut cdc = Lexer::new("-->x");
        assert!(cdc.run().is_ok());
        assert_eq!(cdc.tokens[0].kind, TokenKind::CDC);
        assert_eq!(cdc.peek(), Some('>'));
        cdc.advance();
        assert_eq!(cdc.peek(), Some('x'));
    }

    #[test]
    fn number_consumer_rejects_non_number_starts_without_consumption() {
        for input in ["", "+", "-", ".", "+.", "-.", "--1", "+foo"] {
            let mut lexer = Lexer::new(input);

            let result = lexer.number(false);
            assert!(matches!(
                result,
                Err(LexerError {
                    reason: LexerErrorReason::MATCHED_PREFIX,
                    ..
                })
            ), "input: {input}");
            assert_eq!(lexer.current, 0, "input: {input}");
            assert!(lexer.last_size_memo.is_empty(), "input: {input}");
        }
    }

    #[test]
    fn closed_comment_stops_on_the_comment_terminator() {
        let mut lexer = Lexer::new("/* comment */x");

        assert!(lexer.comment().is_ok());
        assert_eq!(lexer.peek(), Some('/'));
        lexer.advance();
        assert_eq!(lexer.peek(), Some('x'));
    }

    #[test]
    fn unterminated_comment_reports_an_error_at_eof() {
        let mut lexer = Lexer::new("/* comment");

        let result = lexer.comment();
        assert!(matches!(
            result,
            Err(LexerError {
                reason: LexerErrorReason::UNTERMINATED_TOKEN,
                ..
            })
        ));
        assert!(lexer.at_end());
    }

    #[test]
    fn function_consumer_stops_on_the_opening_parenthesis() {
        let mut lexer = Lexer::new("calc(");

        assert!(lexer.function(true).is_ok());
        assert_eq!(lexer.peek(), Some('('));
        assert_eq!(lexer.tokens.len(), 1);
        assert_eq!(lexer.tokens[0].kind, TokenKind::FUNCTION);

        lexer.advance();
        assert!(lexer.at_end());
    }

    #[test]
    fn function_consumer_restores_state_when_no_parenthesis_follows() {
        let mut lexer = Lexer::new("calc ");

        let result = lexer.function(false);
        assert!(matches!(
            result,
            Err(LexerError {
                reason: LexerErrorReason::MATCHED_PREFIX,
                ..
            })
        ));
        assert_eq!(lexer.current, 0);
        assert!(lexer.last_size_memo.is_empty());
    }

    #[test]
    fn at_keyword_consumer_stops_on_the_last_identifier_code_point() {
        let mut lexer = Lexer::new("@media{");

        assert!(lexer.at_keyword(true).is_ok());
        assert_eq!(lexer.peek(), Some('a'));
        assert_eq!(lexer.tokens.len(), 1);
        assert_eq!(lexer.tokens[0].kind, TokenKind::AT_KEYWORD);

        lexer.advance();
        assert_eq!(lexer.peek(), Some('{'));
    }

    #[test]
    fn at_keyword_consumer_restores_state_when_identifier_does_not_follow() {
        let mut lexer = Lexer::new("@ ");

        let result = lexer.at_keyword(false);
        assert!(matches!(
            result,
            Err(LexerError {
                reason: LexerErrorReason::MATCHED_PREFIX,
                ..
            })
        ));
        assert_eq!(lexer.current, 0);
        assert!(lexer.last_size_memo.is_empty());
    }
}
