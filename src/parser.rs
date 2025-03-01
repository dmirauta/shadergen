//! Likely a fairly naive parser, but it does the job.

use lazy_static::lazy_static;
use std::collections::{HashMap, HashSet};

use crate::tokeniser::{GTokenError, GTokenKind, TokenSpan, TokenStream};

#[derive(Debug)]
pub struct RewriteRule {
    pub branches: Vec<Branch>,
    /// indeces of branches that replace with a purely terminal rule, we can restrict the choice
    /// to these when we need to cap the depth
    pub terminal_branches: Vec<usize>,
    /// all branches of this rule are terminal
    pub purely_terminal: bool,
}

#[derive(Debug)]
pub struct Branch {
    pub weight: u8,
    pub expr: Expression,
}

// TODO: Would be more efficient to store the AST in an arena, though may not really matter
// in this simple case

#[derive(Debug, Clone)]
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
}

impl Expression {
    fn get_replace_rule(&self) -> String {
        match self {
            Expression::ToBeReplaced { rule } => rule.clone(),
            _ => panic!("This method can only be used on replace variants."),
        }
    }
}

// TODO: runtime registration of new funcs? (since they can be typed into the shader editor)
lazy_static! {
    static ref FUNCTION_WHITELIST: HashMap<&'static str, usize> = [
        ("abs", 1),
        ("exp", 1),
        ("sqrt", 1),
        ("sin", 1),
        ("add", 2),
        ("mult", 2),
        ("sig", 3)
    ]
    .into_iter()
    .collect();
}

#[derive(Debug, Clone)]
pub enum Term {
    // TODO: allow for specifying random number range in grammar, perhaps with square brackets?
    RandConst,
    /// horizontal parameter ranging in [0,1]
    U,
    /// vertical parameter ranging in [0,1]
    V,
    /// time
    T,
    /// radius from screen center, i.e. sqrt(u^2 + v^2)
    R,
    // TODO: add a numeric literal variant? (requires tokeniser change)
}

impl Term {
    fn from_str(ident: &str) -> Option<Self> {
        match ident {
            "rand" | "random" => Some(Self::RandConst),
            "u" => Some(Self::U),
            "v" => Some(Self::V),
            "t" => Some(Self::T),
            "r" => Some(Self::R),
            _ => None,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum ParseFail {
    TokeniserErr(GTokenError),
    BadArglist,
    UnterminatedRule,
    EmptyExpression,
    ExpectedIdentifier,
    UnsupportedNumberOfFunctionArgs(usize),
    WrongNumberOfFunctionArgs {
        func: String,
        expected: usize,
        got: usize,
    },
    FunctionNotWhitelisted(String),
    NoRulesFound,
    /// required to be able to limit depth
    NoTerminalReplacementInChannelRule,
    ExpectedBars,
}

impl From<GTokenError> for ParseFail {
    fn from(value: GTokenError) -> Self {
        Self::TokeniserErr(value)
    }
}

type PResult<T> = Result<T, ParseFail>;

#[derive(Debug)]
struct GToken {
    kind: GTokenKind,
    // TODO: pass through error locations on ParseFail
    #[allow(dead_code)]
    span: TokenSpan,
}

#[allow(dead_code)]
fn dbg_toks(toks: &[GToken]) {
    for tok in toks {
        print!(" {}", tok.kind.as_str());
    }
    println!();
}

#[derive(Debug)]
pub struct RewriteRules {
    pub rules: HashMap<String, RewriteRule>,
    pub entry_point: String,
}

pub fn parse_rewrite_rules(src: &str) -> PResult<RewriteRules> {
    let mut ts = TokenStream::new(src.chars()).peekable();

    let mut rules = HashMap::new();
    let mut entry_point = None;
    let mut purely_terminal = HashSet::new();
    let mut toks = vec![];
    loop {
        loop {
            match ts.next() {
                None => return Err(ParseFail::UnterminatedRule),
                Some((Ok(kind), span)) => match kind {
                    GTokenKind::Colon => break,
                    _ => toks.push(GToken { kind, span }),
                },
                Some((Err(e), _)) => return Err(ParseFail::TokeniserErr(e)),
            };
        }
        let (ident, rule) = parse_rewrite_rule(&toks)?;

        if entry_point.is_none() {
            // NOTE: First rule becomes the color channel rule
            entry_point = Some(ident.clone());
        }

        if rule.purely_terminal {
            purely_terminal.insert(ident.clone());
        }
        rules.insert(ident, rule);

        if ts.peek().is_none() {
            break;
        }
        toks.clear();
    }

    for rule in rules.values_mut() {
        let filtered_tb: Vec<_> = rule
            .terminal_branches
            .iter()
            .filter(|i| {
                let tr = rule.branches[**i].expr.get_replace_rule();
                purely_terminal.contains(&tr)
            })
            .cloned()
            .collect();
        rule.terminal_branches = filtered_tb;
    }

    match entry_point {
        Some(entry_point) => match rules
            .get(&entry_point)
            .unwrap()
            .terminal_branches
            .is_empty()
        {
            true => Err(ParseFail::NoTerminalReplacementInChannelRule),
            false => Ok(RewriteRules { rules, entry_point }),
        },
        None => Err(ParseFail::NoRulesFound),
    }
}

fn parse_rewrite_rule(toks: &[GToken]) -> PResult<(String, RewriteRule)> {
    let n = toks.len();
    let rule_ident = match &toks[0].kind {
        GTokenKind::Ident { name } => name.clone(),
        _ => return Err(ParseFail::ExpectedIdentifier),
    };

    let mut branches = vec![];
    let mut terminal_branches = vec![];
    let mut purely_terminal = true;
    let mut i = 1;
    let mut j = 1;
    loop {
        // skip initial bars
        while toks[j].kind == GTokenKind::Bar {
            j += 1;
        }
        // seek to end of branch
        while j < n && toks[j].kind != GTokenKind::Bar {
            j += 1;
        }
        let branch = parse_branch(&toks[i..j])?;
        if matches!(&branch.expr, Expression::ToBeReplaced { .. }) {
            // NOTE: at this point these are just candidates, they need to be filtered
            terminal_branches.push(branches.len());
        }
        if !matches!(&branch.expr, Expression::Terminal { .. }) {
            purely_terminal = false;
        }
        branches.push(branch);
        if j == n {
            break;
        }
        i = j;
    }
    let out = (
        rule_ident,
        RewriteRule {
            branches,
            terminal_branches,
            purely_terminal,
        },
    );
    Ok(out)
}

fn parse_branch(toks: &[GToken]) -> PResult<Branch> {
    let mut i = 0;
    let mut weight = 0;
    while toks[i].kind == GTokenKind::Bar {
        weight += 1;
        i += 1;
    }
    if weight == 0 {
        Err(ParseFail::ExpectedBars)
    } else {
        Ok(Branch {
            weight,
            expr: parse_expr(&toks[i..])?,
        })
    }
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

fn parse_expr(toks: &[GToken]) -> PResult<Expression> {
    let n = toks.len();
    if n == 0 {
        return Err(ParseFail::EmptyExpression);
    }
    let ident = match &toks[0].kind {
        GTokenKind::Ident { name } => name.clone(),
        _ => return Err(ParseFail::ExpectedIdentifier),
    };
    if n == 1 {
        match Term::from_str(&ident) {
            Some(term) => Ok(Expression::Terminal(term)),
            None => Ok(Expression::ToBeReplaced { rule: ident }),
        }
    } else if toks[1].kind == GTokenKind::LPar && toks[n - 1].kind == GTokenKind::RPar {
        let argss = split_arglist(&toks[2..n - 1]);
        let mut args = vec![];
        for s in argss.into_iter() {
            let arg = parse_expr(s)?;
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
        Ok(expr)
    } else {
        Err(ParseFail::BadArglist)
    }
}

fn split_arglist(toks: &[GToken]) -> Vec<&[GToken]> {
    let mut j = 0;
    let mut k = 0;
    let mut args = vec![];
    let mut level: i32 = 0;
    while k < toks.len() {
        if toks[k].kind == GTokenKind::LPar {
            level += 1;
        }
        if toks[k].kind == GTokenKind::RPar {
            level -= 1;
        }
        if level == 0 && toks[k].kind == GTokenKind::Comma {
            args.push(&toks[j..k]);
            j = k + 1;
        }
        k += 1;
    }
    args.push(&toks[j..]);
    args
}
