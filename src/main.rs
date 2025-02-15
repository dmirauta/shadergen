use parser::parse_rewrite_rules;
use std::fs::read_to_string;

mod parser;

fn main() {
    let grammar_bytes = read_to_string("grammar.bnf").unwrap();
    let rules = parse_rewrite_rules(grammar_bytes.as_bytes()).unwrap();
    dbg!(rules);
}
