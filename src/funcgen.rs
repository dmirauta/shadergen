use lazy_static::lazy_static;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::parser::{Expression, RewriteRule, RewriteRules};
use std::{collections::HashMap, fmt::Write, sync::RwLock};

fn weighted_pick(weights: &[u16], cidx: u16) -> Option<usize> {
    let cumsum: Vec<u16> = (0..=weights.len())
        .map(|i| weights[..i].iter().sum())
        .collect();
    cumsum
        .windows(2)
        .enumerate()
        .find(|(_, w)| w[0] <= cidx && cidx < w[1])
        .map(|(i, _)| i)
}

lazy_static! {
    pub static ref SRNG: RwLock<ChaCha8Rng> = RwLock::new(ChaCha8Rng::seed_from_u64(0));
    pub static ref RNG: RwLock<ChaCha8Rng> = RwLock::new(ChaCha8Rng::seed_from_u64(0));
}

impl RewriteRule {
    pub fn choose_random(&self) -> Expression {
        let weights: Vec<u16> = self.branches.iter().map(|b| b.weight as u16).collect();
        let weights_total: u16 = weights.iter().cloned().sum();
        let mut rcidx: u16 = RNG.write().unwrap().random();
        rcidx %= weights_total;
        let ridx = weighted_pick(&weights, rcidx).unwrap();
        self.branches[ridx].expr.clone()
    }
    fn choose_terminal(&self, rules: &HashMap<String, RewriteRule>) -> Expression {
        let rii: u8 = RNG.write().unwrap().random();
        let rii = (rii as usize) % self.terminal_branches.len();
        // TODO: ^effectively using uniform weights here rather than whats defined in the grammar...
        let ridx = self.terminal_branches[rii];
        if let Expression::ToBeReplaced { rule } = &self.branches[ridx].expr {
            rules.get(rule).unwrap().choose_random()
        } else {
            unreachable!();
        }
    }
}

impl RewriteRules {
    // TODO: warn when initial expressions are already deeper than max_depth, or ignore?
    pub fn replace_leafs(
        &self,
        base_expr: Box<Expression>,
        max_depth: usize,
    ) -> (Box<Expression>, usize) {
        let mut leafs = vec![];
        let mut queue = vec![(&base_expr, 1)];
        // NOTE: we are re-traversing on every iteration, ideally we would add new leafs as we added
        // subtrees
        while let Some((next, depth)) = queue.pop() {
            match next.as_ref() {
                Expression::Terminal(..) => {}
                Expression::Func1 { args, .. } => {
                    for arg in args {
                        queue.push((arg, depth + 1));
                    }
                }
                Expression::Func2 { args, .. } => {
                    for arg in args {
                        queue.push((arg, depth + 1));
                    }
                }
                Expression::Func3 { args, .. } => {
                    for arg in args {
                        queue.push((arg, depth + 1));
                    }
                }
                Expression::ToBeReplaced { rule } => {
                    leafs.push((next, depth + 1, rule.clone()));
                }
            }
        }
        let n_leafs = leafs.len();
        while let Some((leaf, depth, rule)) = leafs.pop() {
            let leafp = leaf as *const Box<Expression> as *mut Box<Expression>;
            let new_expr = if depth < max_depth {
                self.rules.get(&rule).unwrap().choose_random()
            } else {
                self.rules.get(&rule).unwrap().choose_terminal(&self.rules)
            };
            // NOTE: seems clearly safe from a non-concurrent acces point of view, but should double
            // check it does not cause leaks
            let leafr = unsafe { leafp.as_mut() }.unwrap();
            **leafr = new_expr;
        }
        (base_expr, n_leafs)
    }

    // TODO: regenerate if depth is too low (e.g. starting with terminal)
    pub fn gen_fn(&self, max_depth: usize) -> Box<Expression> {
        let mut func = Box::new(self.rules.get(&self.entry_point).unwrap().choose_random());
        loop {
            let (func_, leafs) = self.replace_leafs(func, max_depth);
            func = func_;
            if leafs == 0 {
                break;
            }
        }
        func
    }
}

impl Expression {
    // TODO: non-recursive impl?
    pub fn as_string(&self) -> String {
        let mut buff = String::new();
        match self {
            Expression::Terminal(term) => match term {
                crate::parser::Term::RandConst => {
                    let r: f32 = RNG.write().unwrap().random();
                    _ = buff.write_fmt(format_args!("{r:.2}"));
                }
                crate::parser::Term::U => _ = buff.write_str("u"),
                crate::parser::Term::V => _ = buff.write_str("v"),
                crate::parser::Term::T => _ = buff.write_str("t"),
                crate::parser::Term::R => _ = buff.write_str("r"),
            },
            Expression::Func1 { ident, args } => {
                _ = buff.write_fmt(format_args!("{ident}({})", args[0].as_string()));
            }
            Expression::Func2 { ident, args } => {
                _ = buff.write_fmt(format_args!(
                    "{ident}({},{})",
                    args[0].as_string(),
                    args[1].as_string()
                ));
            }
            Expression::Func3 { ident, args } => {
                _ = buff.write_fmt(format_args!(
                    "{ident}({},{},{})",
                    args[0].as_string(),
                    args[1].as_string(),
                    args[2].as_string()
                ));
            }
            Expression::ToBeReplaced { .. } => {
                // TODO: log warn that these should be replaced by now..?
                _ = buff.write_str("_");
            }
        }
        buff
    }
}
