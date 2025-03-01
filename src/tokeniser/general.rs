use std::marker::PhantomData;

#[derive(Clone, Copy, Debug)]
pub struct SourceLoc {
    /// number of lines down
    pub line: u16,
    /// number of chars across
    pub char: u8,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub struct TokenSpan {
    pub line: u16,
    pub startchar: u8,
    pub length: u8,
}

pub static WHITESPACE: [char; 3] = [' ', '\t', '\n'];

/// interface for interacting with the char stream held by a token stream
pub trait ExposesCharstream {
    fn current(&self) -> Option<char>;
    fn advance(&mut self);
    fn skip_whitespace(&mut self) {
        while let Some(c) = self.current() {
            if WHITESPACE.contains(&c) {
                self.advance();
            } else {
                return;
            }
        }
    }
    fn seek_to_next_line(&mut self) {
        while let Some(c) = self.current() {
            self.advance();
            if c == '\n' {
                return;
            }
        }
    }
}

/// a call to Token::recognise will consume a number of tokens, the last `length` of which belong
/// to the token (in case of an error, this may represent a partial token)
pub enum RecognitionResult<T, E> {
    NextToken { token: T, length: u8 },
    Eof,
    Err { error: E, length: u8 },
}

pub trait TokenRec
where
    Self: Sized,
{
    type RecognitionError;

    fn recognise<CS: ExposesCharstream>(
        cs: &mut CS,
    ) -> RecognitionResult<Self, Self::RecognitionError>;
}

pub struct TokenStream<TokenKind, CharStream: Iterator<Item = char>> {
    charstream: CharStream,
    current: Option<char>,
    cursor: SourceLoc,
    // NOTE: the token gets placed past the end of a token, so the cursor may have wrapped
    prev_lastchar: u8,
    _p: PhantomData<TokenKind>,
}

impl<CharStream, TokenKind> TokenStream<TokenKind, CharStream>
where
    CharStream: Iterator<Item = char>,
    TokenKind: TokenRec,
{
    pub fn new(mut charstream: CharStream) -> Self {
        let current = charstream.next();
        let cursor = SourceLoc { line: 0, char: 0 };
        Self {
            charstream,
            current,
            cursor,
            prev_lastchar: 0,
            _p: Default::default(),
        }
    }
    fn length2span(&self, length: u8) -> TokenSpan {
        match self.cursor.char.checked_sub(length) {
            Some(startchar) => TokenSpan {
                line: self.cursor.line,
                startchar,
                length,
            },
            None => {
                if self.cursor.char == 0 && length == 1 {
                    TokenSpan {
                        line: self.cursor.line - 1,
                        startchar: self.prev_lastchar - 1,
                        length,
                    }
                } else {
                    panic!("Likely error in TokenRec impl: given token length is {length} but the ammount of chars on the current line is {}.", self.cursor.char)
                }
            }
        }
    }
}

impl<CharStream, TokenKind> ExposesCharstream for TokenStream<TokenKind, CharStream>
where
    CharStream: Iterator<Item = char>,
    TokenKind: TokenRec,
{
    fn advance(&mut self) {
        // TODO: check & communicate max lines (~65K per source file) and max chars per line (255)
        match self.current {
            Some('\n') => {
                self.prev_lastchar = self.cursor.char;
                self.cursor.line += 1;
                self.cursor.char = 0;
            }
            Some(_) => {
                self.cursor.char += 1;
            }
            _ => {}
        }
        self.current = self.charstream.next();
    }

    fn current(&self) -> Option<char> {
        self.current
    }
}

impl<CharStream, TokenKind> Iterator for TokenStream<TokenKind, CharStream>
where
    CharStream: Iterator<Item = char>,
    TokenKind: TokenRec + std::fmt::Debug,
{
    type Item = (Result<TokenKind, TokenKind::RecognitionError>, TokenSpan);

    // NOTE: once a TokenRec err is emmited, it will likely repeat infinitely
    fn next(&mut self) -> Option<Self::Item> {
        match TokenKind::recognise(self) {
            RecognitionResult::NextToken { token, length } => {
                Some((Ok(token), self.length2span(length)))
            }
            RecognitionResult::Eof => None,
            RecognitionResult::Err { error, length } => {
                // TODO: might be useful to have a multiline span on error?
                Some((Err(error), self.length2span(length)))
            }
        }
    }
}

#[allow(dead_code)]
pub fn highlight_token(src: &str, span: TokenSpan, cursor: char) {
    if let Some(line) = src.lines().nth(span.line as usize) {
        println!("{line}");
        for _ in 0..span.startchar {
            print!(" ");
        }
        for _ in 0..span.length {
            print!("{cursor}");
        }
        println!();
    }
}

#[macro_export]
macro_rules! recog_single_char_token {
    ($cs:ident, $variant: expr) => {{
        $cs.advance();
        RecognitionResult::NextToken {
            token: $variant,
            length: 1,
        }
    }};
}
