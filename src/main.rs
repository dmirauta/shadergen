use generator::gen_fn;
use parser::parse_rewrite_rules;
use std::fs::read_to_string;

mod generator;
mod parser;

fn main() {
    let grammar_bytes = read_to_string("grammar.bnf").unwrap();
    let (rules, crule) = parse_rewrite_rules(grammar_bytes.as_bytes()).unwrap();
    // dbg!(&crule, &rules);

    let func = gen_fn(crule, &rules, 15);
    func.compact_print();
    println!();
}
