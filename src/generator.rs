use crate::parser::{Expression, RewriteRule};
use rand::{rng, Rng};
use std::collections::HashMap;

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

impl RewriteRule {
    pub fn choose_random(&self) -> Expression {
        let weights: Vec<u16> = self.branches.iter().map(|b| b.weight as u16).collect();
        let weights_total: u16 = weights.iter().cloned().sum();
        let mut rcidx: u16 = rng().random();
        rcidx %= weights_total;
        let ridx = weighted_pick(&weights, rcidx).unwrap();
        self.branches[ridx].expr.clone()
    }
    fn choose_terminal(&self, rules: &HashMap<String, RewriteRule>) -> Expression {
        if self.terminal_branches.len() > 1 {
            println!("Multiple terminal branches found in channel rule, but currently always picking the first (that is referenced in the channel rule).");
            //TODO: address above
        }
        let ridx = self.terminal_branches[0];
        if let Expression::ToBeReplaced { rule } = &self.branches[ridx].expr {
            rules.get(rule).unwrap().choose_random()
        } else {
            unreachable!();
        }
    }
}

// TODO: warn when initial expressions are already deeper than max_depth, or ignore?
pub fn replace_leafs(
    base_expr: Box<Expression>,
    rules: &HashMap<String, RewriteRule>,
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
            rules.get(&rule).unwrap().choose_random()
        } else {
            rules.get(&rule).unwrap().choose_terminal(rules)
        };
        // NOTE: seems clearly safe from a non-concurrent acces point of view, but should double
        // check it does not cause leaks
        let leafr = unsafe { leafp.as_mut() }.unwrap();
        **leafr = new_expr;
    }
    (base_expr, n_leafs)
}

pub fn gen_fn(
    crule: String,
    rules: &HashMap<String, RewriteRule>,
    max_depth: usize,
) -> Box<Expression> {
    let mut func = Box::new(rules.get(&crule).unwrap().choose_random());
    loop {
        let (func_, leafs) = replace_leafs(func, rules, max_depth);
        func = func_;
        if leafs == 0 {
            break;
        }
    }
    func
}

impl Expression {
    // TODO: non-recursive impl?
    pub fn compact_print(&self) {
        match self {
            Expression::Terminal(term) => match term {
                crate::parser::Term::RandConst => {
                    let r: f32 = rng().random();
                    print!("{r:.2}");
                }
                crate::parser::Term::U => print!("u"),
                crate::parser::Term::V => print!("v"),
                crate::parser::Term::T => print!("t"),
                crate::parser::Term::R => print!("r"),
            },
            Expression::Func1 { ident, args } => {
                print!("{ident}(");
                args[0].compact_print();
                print!(")");
            }
            Expression::Func2 { ident, args } => {
                print!("{ident}(");
                args[0].compact_print();
                print!(",");
                args[1].compact_print();
                print!(")");
            }
            Expression::Func3 { ident, args } => {
                print!("{ident}(");
                args[0].compact_print();
                print!(",");
                args[1].compact_print();
                print!(",");
                args[2].compact_print();
                print!(")");
            }
            Expression::ToBeReplaced { .. } => {
                // TODO: log warn that these should be replaced by now..?
                print!("_");
            }
        }
    }
}
