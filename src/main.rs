use parser::parse_rewrite_rule;
use std::fs::read_to_string;

mod parser;

fn main() {
    let grammar_bytes = read_to_string("grammar.bnf").unwrap();
    let (_, rule) = parse_rewrite_rule(grammar_bytes.as_bytes()).unwrap();
    dbg!(rule);
}
