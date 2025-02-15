//! Likely a very bad (and certainly ugly) parser, but it does the job.

// TODO: Should probably use &str rather than &[u8], to support unicode, but its nice being able
// to index the slice by a usize...
// TODO: either offload more work to nom or remove it? (could just implement the few functions used)

use lazy_static::lazy_static;
use nom::{
    bytes::{complete::take_while1, take_until},
    character::{char, complete::alphanumeric0},
    AsChar, Parser,
};
use std::collections::HashMap;

#[allow(dead_code)]
#[derive(Debug)]
pub struct RewriteRule(Vec<Branch>);

#[allow(dead_code)]
#[derive(Debug)]
pub struct Branch {
    weight: u8,
    expr: Expression,
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum Expression {
    Terminal(Term),
    Func1 {
        ident: String,
        args: [Box<Expression>; 1],
    },
    Func2 {
        ident: String,
        args: [Box<Expression>; 2],
    },
    Func3 {
        ident: String,
        args: [Box<Expression>; 3],
    },
    ToBeReplaced {
        rule: String,
    },
    Debug(String),
}

lazy_static! {
    static ref FUNCTION_WHITELIST: HashMap<&'static str, usize> =
        [("abs", 1), ("sqrt", 1), ("sin", 1), ("add", 2), ("mult", 2),]
            .into_iter()
            .collect();
}

#[derive(Debug)]
pub enum Term {
    RandConst,
    /// horizontal parameter ranging in [0,1]
    U,
    /// vertical parameter ranging in [0,1]
    V,
    /// time
    T,
    /// radius from screen center, sqrt(add(mult(u-0.5, u-0.5), mult(v-0.5, v-0.5)))
    R,
    // TODO: add literal? (issue: variants currently derived from alphanumeric str)
}

impl Term {
    fn from_str(ident: &str) -> Option<Self> {
        match ident.to_lowercase().as_str() {
            "rand" | "random" => Some(Self::RandConst),
            "u" => Some(Self::U),
            "v" => Some(Self::V),
            "t" => Some(Self::T),
            "r" => Some(Self::R),
            _ => None,
        }
    }
}

fn _strip_comment(i: &[u8]) -> Option<&[u8]> {
    (char::<_, ()>('#'), take_until("\n"), char('\n'))
        .parse(i)
        .ok()
        .map(|(i, _)| i)
}

fn strip_comment(i: &[u8]) -> &[u8] {
    _strip_comment(i).unwrap_or(i)
}

static WHITESPACE_CHARS: [char; 3] = [' ', '\t', '\n'];

fn _eat_whitespace(i: &[u8]) -> Option<&[u8]> {
    take_while1::<_, _, ()>(|c: u8| WHITESPACE_CHARS.contains(&c.as_char()))(i)
        .ok()
        .map(|(i, _)| i)
}

fn eat_whitespace(i: &[u8]) -> &[u8] {
    _eat_whitespace(i).unwrap_or(i)
}

fn eat_whitespace_and_comments(mut i: &[u8]) -> &[u8] {
    if i.is_empty() {
        return i;
    }
    while [' ', '\t', '\n', '#'].contains(&i[0].as_char()) {
        i = eat_whitespace(i);
        i = strip_comment(i);
        if i.is_empty() {
            break;
        }
    }
    i
}

#[test]
fn test_eat_whitespace_and_comments() {
    let i = r"
    # comment 1


    # another
    something else";
    let o = eat_whitespace_and_comments(i.as_bytes());
    assert_eq!(o, "something else".as_bytes());
    let i = r"no comment";
    let o = eat_whitespace_and_comments(i.as_bytes());
    assert_eq!(o, "no comment".as_bytes())
}

fn utf8str(i: &[u8]) -> String {
    String::from_utf8(i.to_vec()).unwrap()
}

/// returns the specified result, as well as leftover bytes (but not on error)
type PResult<'a, T> = Result<(&'a [u8], T), ParseFail>;

#[allow(dead_code)]
#[derive(Debug)]
pub enum ParseFail {
    EmptyExpression,
    ExpectedIdentifier,
    NoClosingBracket,
    UnexpectedCharsAfterClosingBracket,
    UnsupportedNumberOfFunctionArgs(usize),
    WrongNumberOfFunctionArgs {
        func: String,
        expected: usize,
        got: usize,
    },
    FunctionNotWhitelisted(String),
}

pub fn parse_rewrite_rules(mut i: &[u8]) -> Result<HashMap<String, RewriteRule>, ParseFail> {
    i = eat_whitespace_and_comments(i);
    let mut hs = HashMap::new();
    while !i.is_empty() {
        let (i_, (ident, rule)) = parse_rewrite_rule(i)?;
        i = eat_whitespace_and_comments(i_);
        hs.insert(ident, rule);
    }
    Ok(hs)
}

fn parse_rewrite_rule(mut i: &[u8]) -> PResult<(String, RewriteRule)> {
    let (i_, rule_ident) = alphanumeric0::<_, ()>(i).map_err(|_| ParseFail::ExpectedIdentifier)?;
    let rule_ident = utf8str(rule_ident);
    dbg!(&rule_ident);
    i = i_;

    let mut branches = vec![];
    loop {
        i = eat_whitespace_and_comments(i);
        match char::<_, ()>(';').parse(i) {
            Ok((i_, _)) => {
                i = i_;
                break;
            }
            Err(_) => {
                let (i_, branch) = parse_branch(i)?;
                i = i_;
                branches.push(branch);
            }
        }
    }
    let out = (rule_ident, RewriteRule(branches));
    Ok((i, out))
}

fn parse_branch(i: &[u8]) -> PResult<Branch> {
    let (mut i, bars) = take_while1::<_, _, ()>(|u: u8| u.as_char() == '|')
        .parse(i)
        .unwrap();
    let weight = bars.len() as u8;
    i = eat_whitespace_and_comments(i);
    let (mut i, in_branch) =
        take_while1::<_, _, ()>(|u: u8| !['|', ';', '#'].contains(&u.as_char()))
            .parse(i)
            .map_err(|_| ParseFail::EmptyExpression)?;
    i = eat_whitespace_and_comments(i);
    Ok((
        i,
        Branch {
            weight,
            expr: parse_expr(in_branch)?.1,
        },
    ))
}

macro_rules! check_function {
    ($id: expr, $given_nargs: tt) => {
        match FUNCTION_WHITELIST.get(&$id.as_str()) {
            Some(&expected) => {
                if expected != $given_nargs {
                    return Err(ParseFail::WrongNumberOfFunctionArgs {
                        func: $id,
                        expected,
                        got: $given_nargs,
                    });
                }
            }
            None => {
                return Err(ParseFail::FunctionNotWhitelisted($id));
            }
        }
    };
}

fn parse_expr(mut i: &[u8]) -> PResult<Expression> {
    i = eat_whitespace_and_comments(i);
    let (i_, ident) = alphanumeric0::<_, ()>(i).unwrap();
    let ident = utf8str(ident);
    if ident.is_empty() {
        return Err(ParseFail::ExpectedIdentifier);
    }
    i = i_;
    match parse_brackets_inner(i) {
        ParseBracketsOutcome::NoBrackets => match Term::from_str(&ident) {
            Some(term) => Ok((i, Expression::Terminal(term))),
            None => Ok((i, Expression::ToBeReplaced { rule: ident })),
        },
        ParseBracketsOutcome::Success(i) => {
            let argsi = split_arglist(i);
            let mut args = vec![];
            for (j, i) in argsi.into_iter().enumerate() {
                println!("Invalid expr as arg {j} of {ident}"); // TODO: Log more verbose info
                let (_, arg) = parse_expr(i)?;
                args.push(Box::new(arg))
            }
            let expr = match args.len() {
                1 => {
                    check_function!(ident, 1);
                    Expression::Func1 {
                        ident,
                        args: [args.pop().unwrap()],
                    }
                }
                2 => {
                    check_function!(ident, 2);
                    let arg2 = args.pop().unwrap();
                    let arg1 = args.pop().unwrap();
                    Expression::Func2 {
                        ident,
                        args: [arg1, arg2],
                    }
                }
                3 => {
                    check_function!(ident, 3);
                    let arg3 = args.pop().unwrap();
                    let arg2 = args.pop().unwrap();
                    let arg1 = args.pop().unwrap();
                    Expression::Func3 {
                        ident,
                        args: [arg1, arg2, arg3],
                    }
                }
                n => return Err(ParseFail::UnsupportedNumberOfFunctionArgs(n)),
            };
            Ok((i, expr))
        }
        ParseBracketsOutcome::NoClosing => Err(ParseFail::NoClosingBracket),
        ParseBracketsOutcome::ExtraTokens => Err(ParseFail::UnexpectedCharsAfterClosingBracket),
    }
}

fn parse_brackets_inner(mut i: &[u8]) -> ParseBracketsOutcome {
    i = eat_whitespace_and_comments(i);
    i = match char::<_, ()>('(').parse(i) {
        Ok((i, _)) => i,
        Err(_) => return ParseBracketsOutcome::NoBrackets,
    };
    let mut j = i.len() - 1;
    while j > 0 {
        if i[j].as_char() == ')' {
            if i[j + 1..]
                .iter()
                .any(|u| !WHITESPACE_CHARS.contains(&u.as_char()))
            {
                return ParseBracketsOutcome::ExtraTokens;
            }
            return ParseBracketsOutcome::Success(&i[..j]);
        }
        j -= 1;
    }
    ParseBracketsOutcome::NoClosing
}

#[derive(Debug)]
enum ParseBracketsOutcome<'a> {
    NoBrackets,
    NoClosing,
    ExtraTokens,
    Success(&'a [u8]),
}

fn split_arglist(i: &[u8]) -> Vec<&[u8]> {
    let mut j = 0;
    let mut k = 0;
    let mut args = vec![];
    let mut level: i32 = 0;
    while k < i.len() {
        if i[k].as_char() == '(' {
            level += 1;
        }
        if i[k].as_char() == ')' {
            level -= 1;
        }
        if level == 0 && i[k].as_char() == ',' {
            args.push(&i[j..k]);
            j = k + 1;
        }
        k += 1;
    }
    args.push(&i[j..]);
    args
}
