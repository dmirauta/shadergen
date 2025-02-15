//! Likely a very bad (and certainly ugly) parser, but it does the job.

// TODO: Should probably use &str rather than &[u8], to support unicode, but its nice being able
// to index the slice by a usize...

use std::collections::HashMap;

use lazy_static::lazy_static;
use nom::{
    bytes::{complete::take_while1, take_until},
    character::{char, complete::alphanumeric0},
    AsChar, IResult, Parser,
};

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
    U,
    V,
    T,
    R, // sqrt(add(mult(u-0.5, u-0.5), mult(v-0.5, v-0.5)))
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

/// returns the specified result, as well as leftover bytes (including on error)
type PResult<'a, T> = IResult<&'a [u8], T>;

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

// TODO: replace panics/expects with recoverable feedback (possibly with logs?)

fn utf8str(i: &[u8]) -> String {
    String::from_utf8(i.to_vec()).unwrap()
}

pub fn parse_rewrite_rule(mut i: &[u8]) -> PResult<(String, RewriteRule)> {
    i = eat_whitespace_and_comments(i);
    let (i_, rule_ident) = alphanumeric0::<_, ()>(i).expect("Expected rule name.");
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
                let (i_, branch) = parse_branch(i).expect("Invalid branch or missing \";\".");
                i = i_;
                branches.push(branch);
            }
        }
    }
    let out = (rule_ident, RewriteRule(branches));
    Ok((i, out))
}

fn parse_branch(i: &[u8]) -> PResult<Branch> {
    let (i, bars) = take_while1(|u: u8| u.as_char() == '|').parse(i)?;
    let weight = bars.len() as u8;
    let (mut i, in_branch) =
        take_while1(|u: u8| !['|', ';', '#'].contains(&u.as_char())).parse(i)?;
    i = eat_whitespace_and_comments(i);
    Ok((
        i,
        Branch {
            weight,
            expr: parse_expr(in_branch)?.1,
        },
    ))
}

fn check_function(ident: &str, given_nargs: usize) {
    match FUNCTION_WHITELIST.get(&ident) {
        Some(nargs) => {
            if *nargs != given_nargs {
                panic!("\"{ident}\" requires {nargs} arguments but {given_nargs} were given.");
            }
        }
        None => {
            panic!("\"{ident}\" is not whitelisted (may or may not exist in glsl).")
        }
    }
}

fn parse_expr(mut i: &[u8]) -> PResult<Expression> {
    i = eat_whitespace_and_comments(i);
    let (i_, ident) =
        alphanumeric0::<_, ()>(i).expect("Expected the expression to start with an identifier.");
    let ident = utf8str(ident);
    if ident.is_empty() {
        panic!("Empty rule found (will ignore).");
    }
    i = i_;
    match parse_brackets_inner(i) {
        ParseBracketsOutcome::NoBrackets => match Term::from_str(&ident) {
            Some(term) => Ok((i, Expression::Terminal(term))),
            None => Ok((i, Expression::ToBeReplaced { rule: ident })),
        },
        ParseBracketsOutcome::Success(i) => {
            let args = split_arglist(i);
            let mut args: Vec<_> = args
                .into_iter()
                .enumerate()
                .map(|(j, i)| {
                    let (_, arg) = parse_expr(i)
                        .unwrap_or_else(|_| panic!("Invalid expr as arg {j} of {ident}"));
                    Box::new(arg)
                })
                .collect();
            let expr = match args.len() {
                1 => {
                    check_function(ident.as_str(), 1);
                    Expression::Func1 {
                        ident,
                        args: [args.pop().unwrap()],
                    }
                }
                2 => {
                    check_function(ident.as_str(), 2);
                    let arg2 = args.pop().unwrap();
                    let arg1 = args.pop().unwrap();
                    Expression::Func2 {
                        ident,
                        args: [arg1, arg2],
                    }
                }
                3 => {
                    check_function(ident.as_str(), 3);
                    let arg3 = args.pop().unwrap();
                    let arg2 = args.pop().unwrap();
                    let arg1 = args.pop().unwrap();
                    Expression::Func3 {
                        ident,
                        args: [arg1, arg2, arg3],
                    }
                }
                n => panic!("functions with {n} arguments are not supported (expecting 1,2 or 3)"),
            };
            Ok((i, expr))
        }
        e => {
            panic!("Parse brackets error: {e:?}")
        }
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
