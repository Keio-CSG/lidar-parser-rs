use ls_pcap_parser_rs::{parseargs::parse_args, run::run};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let args = parse_args("ls-pcap-parser-rs", &args[1..].to_vec());
    run(args);
}