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
        cmp == '\n' || cmp == '\r' && cmp == '\u{000C}'
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
                    x if x.is_whitespace() => { self.whitespace(true)?; },
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
                        if let Some(cmp) = self.peek_next('/') {
                            if cmp == '*'  {
                                self.comment()?;
                                return Ok(());
                            } else {
                                self.add_token(Token::new(TokenKind::SLASH, self.line, Cow::Borrowed("/")));
                            }
                        }
                        self.add_token(Token::new(TokenKind::SLASH, self.line, Cow::Borrowed("/")));
                    },
                    '+' => {
                        self.number(true)?;
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
                            self.current += 3;
                            self.add_token(Token::new(TokenKind::CDO, self.line, Cow::Borrowed("<!--")));
                            return Ok(());
                        }

                        if self.input[self.current..].starts_with("-->") {
                            self.current += 3;
                            self.add_token(Token::new(TokenKind::CDO, self.line, Cow::Borrowed("-->")));
                            return Ok(());
                        }

                        if self.input[self.current..].starts_with("url") {
                            self.current += 3;
                            match self.url(true) {
                                Ok(res) => { return Ok(res)},
                                Err(_) => {
                                    // TODO: backtracking should sync internal state
                                    self.current -= 3;
                                }
                            }

                            return Ok(());
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
                            self.number(true)?;
                            return Ok(())
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
                                }
                            };
                        }

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
        let mut should_align = false;

        if self.catch_match('+') || self.catch_match('-') {
            should_align = true;
        }

        match self.peek() {
            Some(m) if m == '.' => {
                self.advance();
                if let Some(curr) = self.peek() {
                    if curr.is_ascii_digit() {
                        self.advance();
                        self.digit(false);
                        should_align = true;
                    } else {
                        return Err(LexerError::new(
                            LexerErrorReason::UNTERMINATED_TOKEN,
                            self.line,
                            LexerSpan (self.start, self.current),
                        ));
                    }
                }
            }
            Some(d) => {
                if let Some(curr) = self.peek() {
                    if curr.is_ascii_digit() {
                        self.advance();
                        self.digit(false);
                        should_align = true;
                    } else {
                        if should_align {
                            self.step_back();
                        }

                        return Err(LexerError::new(
                            LexerErrorReason::UNTERMINATED_TOKEN,
                            self.line,
                            LexerSpan (self.start, self.current),
                        ));
                    }
                }

                match self.peek() {
                    Some(x) if x == '.' => {
                        self.advance();
                        self.digit(false);
                        should_align = true;
                    }
                    _ => {}
                }

                if self.catch_match('e') || self.catch_match('E') {
                    should_align = true;

                    if !self.catch_match('-') { self.catch_match('+'); }

                    if let Some(curr) = self.peek() {
                        if curr.is_ascii_digit() {
                            self.advance();
                            self.digit(false);
                            should_align = true;
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
            }
            None => {
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

        if should_align {
            self.step_back();

            if collect {
                self.add_token(Token::new(TokenKind::NUMBER, self.line, Cow::Borrowed("")));
            }
        }

        Ok(())
    }

    fn url(&mut self, collect: bool) -> Result<(), LexerError> {
        use utils::is_css_printable;

        let mut should_align = false;
        let branch_point = self.current;
        let memo_point = self.last_size_memo.len();
        let mut current: char;

        if !self.catch_match('(') {
            return Err(LexerError::new(
                LexerErrorReason::NO_MATCH,
                self.line,
                LexerSpan (self.start, self.current),
            ));
        }

        match self.whitespace(false) {
            Ok(_) => {}
            Err(_) => {
                self.backtrack(branch_point, memo_point);
                if self.at_end() {
                    return Err(LexerError::new(
                        LexerErrorReason::UNTERMINATED_TOKEN,
                        self.line,
                        LexerSpan (self.start, self.current),
                    ));
                }
                current = self.peek().unwrap();
                if let Some(next) = self.peek_next(current) {
                    if next == ')' {
                        self.advance();
                        if collect {
                            self.add_token(Token::new(
                                TokenKind::URL,
                                self.line,
                                Cow::Borrowed(""),
                            ));
                        }
                        return Ok(());
                    } else {
                        loop {
                            if self.at_end() {
                                break;
                            }
                            match self.escape(false) {
                                Ok(_) => {
                                    self.advance();
                                    should_align = true;
                                }
                                Err(_) => {
                                    if let Some(next) = self.peek_next(current) {
                                        if matches!(next, '"' | '\'' | '(' | '\\')
                                            || next.is_whitespace()
                                            || !is_css_printable(next)
                                        {
                                            break;
                                        }
                                        self.advance();
                                        should_align = true;
                                        continue;
                                    }
                                }
                            }
                        }
                    }
                } else {
                    return Err(LexerError::new(
                        LexerErrorReason::UNTERMINATED_TOKEN,
                        self.line,
                        LexerSpan (self.start, self.current),
                    ));
                }
            }
        }

        if should_align {
            self.step_back();
        }

        if self.whitespace(false).is_ok() {}

        if self.at_end() {
            return Err(LexerError::new(
                LexerErrorReason::UNTERMINATED_TOKEN,
                self.line,
                LexerSpan (self.start, self.current),
            ));
        }
        current = self.peek().unwrap();
        if let Some(next) = self.peek_next(current) {
            if next == ')' {
                self.advance();
                if collect {
                    self.add_token(Token::new(TokenKind::URL, self.line, Cow::Borrowed("")));
                }

                return Ok(());
            }

            return Err(LexerError::new(
                LexerErrorReason::UNTERMINATED_TOKEN,
                self.line,
                LexerSpan (self.start, self.current),
            ));
        }

        Err(LexerError::new(
            LexerErrorReason::UNTERMINATED_TOKEN,
            self.line,
            LexerSpan (self.start, self.current),
        ))
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
                    Ok(_) => {}
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
                        Ok(_) => {}
                        Err(e) if e.reason == LexerErrorReason::NO_MATCH => {}
                        _ => {
                            // TODO: handle cursor reset
                        }
                    }
                }
                _ => {
                    if should_align {
                        self.step_back();
                        break;
                    }
                }
            }
        }

        if collect {
            self.add_token(Token::new(TokenKind::IDENT, self.line, Cow::Borrowed("")));
        }

        Ok(())
    }

    fn string(&mut self, collect: bool) -> Result<(), LexerError> {
        /*
            TODO: make this compliant with css spec:
              - An unescaped newline should terminate the string as a bad string.
              - Unterminated strings should produce a diagnostic.
        */
        if self.at_end() {
            return Err(LexerError::new(
                LexerErrorReason::INVALID_TOKEN,
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

        loop {
            if self.at_end() {
                return Err(LexerError::new(
                    LexerErrorReason::UNTERMINATED_TOKEN,
                    self.line,
                    LexerSpan (self.start, self.current),
                ));
            }
            self.advance();

            match self.peek() {
                None => {
                    self.step_back();

                    return Err(LexerError::new(
                        LexerErrorReason::UNTERMINATED_TOKEN,
                        self.line,
                        LexerSpan (self.start, self.current),
                    ));
                }
                Some(x) => {
                    match x {
                        c if c == terminal => {
                            if let Some(lb) = self.lookback() {
                                if lb == '\\' {
                                    continue;
                                } else {
                                    if collect {
                                        self.add_token(Token::new(
                                            TokenKind::STRING,
                                            self.line,
                                            Cow::Borrowed(""),
                                        ));
                                    }
                                    return Ok(());
                                }
                            } else {
                                self.step_back();

                                return Err(LexerError::new(
                                    LexerErrorReason::INVARIANT_VIOLATION,
                                    self.line,
                                    LexerSpan (self.start, self.current),
                                ));
                            } //TODO: edge case. raise invariant violation error
                        }
                        n if self.is_newline(n) => {
                            if let Some(lb) = self.lookback() {
                                if lb == '\\' {
                                    continue;
                                } else {
                                    self.step_back();

                                    return Err(LexerError::new(
                                        LexerErrorReason::UNTERMINATED_TOKEN,
                                        self.line,
                                        LexerSpan (self.start, self.current),
                                    ));
                                }
                            } 
                            self.line += 1;
                        }
                        _ => {
                            continue;
                        }
                    }
                }
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
            if !x.is_whitespace() {
                break;
            }
            should_align = true;
            self.newline(x, false);
            self.advance();
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

    fn newline(&mut self, x: char, collect: bool) -> bool {
        let mut dual_seq = false;

        if x == '\r' {
            if let Some(cmp) = self.peek_next('\r') {
                if cmp == '\n' {
                    self.line += 1;
                    self.advance();
                    dual_seq = true;
                }
                if collect {
                    self.add_token(Token::new(
                        TokenKind::WHITESPACE,
                        self.line,
                        Cow::Borrowed(""),
                    ));
                    return dual_seq;
                }
                return dual_seq;
            }
            if collect {
                self.add_token(Token::new(
                    TokenKind::WHITESPACE,
                    self.line,
                    Cow::Borrowed(""),
                ));
                return dual_seq;
            }
            return dual_seq;
        } else if x == '\n' && !dual_seq {
            self.line += 1;
        }

        if collect {
            self.add_token(Token::new(
                TokenKind::WHITESPACE,
                self.line,
                Cow::Borrowed(""),
            ));
        }

        dual_seq
    }
}

mod utils {
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
    fn digit_consumer_stops_on_the_last_digit() {
        let mut lexer = Lexer::new("123x");

        assert!(lexer.digit(false).is_ok());
        assert_eq!(lexer.peek(), Some('3'));
        lexer.advance();
        assert_eq!(lexer.peek(), Some('x'));
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
