use super::general::{ExposesCharstream, RecognitionResult, TokenRec, WHITESPACE};
use crate::recog_single_char_token;

/// Grammar token
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum GTokenKind {
    Ident {
        #[allow(dead_code)]
        name: String,
    },
    /// (
    LPar,
    /// )
    RPar,
    Comma,
    Bar,
    Colon,
}

impl GTokenKind {
    pub fn as_str(&self) -> &str {
        match self {
            GTokenKind::Ident { name } => name,
            GTokenKind::LPar => "(",
            GTokenKind::RPar => ")",
            GTokenKind::Comma => ",",
            GTokenKind::Bar => "|",
            GTokenKind::Colon => ";",
        }
    }
}

#[derive(Debug)]
pub enum GTokenError {
    UnexpectedChar,
}

impl TokenRec for GTokenKind {
    type RecognitionError = GTokenError;

    fn recognise<CS: ExposesCharstream>(
        cs: &mut CS,
    ) -> RecognitionResult<Self, Self::RecognitionError> {
        while let Some(c) = cs.current() {
            if WHITESPACE.contains(&c) {
                cs.skip_whitespace();
            } else if c == '#' {
                cs.seek_to_next_line();
            } else {
                break;
            }
        }
        match cs.current() {
            None => RecognitionResult::Eof,
            Some(',') => recog_single_char_token!(cs, GTokenKind::Comma),
            Some('(') => recog_single_char_token!(cs, GTokenKind::LPar),
            Some(')') => recog_single_char_token!(cs, GTokenKind::RPar),
            Some('|') => recog_single_char_token!(cs, GTokenKind::Bar),
            Some(';') => recog_single_char_token!(cs, GTokenKind::Colon),
            Some(mut c) => {
                let mut name = String::new();
                while c == '_' || c.is_alphanumeric() {
                    name.push(c);
                    cs.advance();
                    c = match cs.current() {
                        Some(c) => c,
                        None => break,
                    };
                }
                // NOTE: should end up here if the first char is not alphanumeric or _
                if name.is_empty() {
                    cs.advance();
                    return RecognitionResult::Err {
                        error: GTokenError::UnexpectedChar,
                        length: 1,
                    };
                }
                RecognitionResult::NextToken {
                    length: name.len() as u8,
                    token: GTokenKind::Ident { name },
                }
            }
        }
    }
}

#[test]
fn tokeniser_test() {
    use super::general::{highlight_token, TokenStream};
    let src = std::fs::read_to_string("grammar.bnf").unwrap();
    let ts = TokenStream::<GTokenKind, _>::new(src.chars());

    println!();
    for (res, span) in ts {
        if let Err(e) = res {
            println!("Err: {e:?}");
            break;
        }
        highlight_token(&src, span, '^');
    }
}
