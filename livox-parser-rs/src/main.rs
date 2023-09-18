use livox_parser_rs::{parseargs::parse_args, run::run};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let args = parse_args("livox-parser-rs", &args[1..].to_vec());
    run(args);
}
