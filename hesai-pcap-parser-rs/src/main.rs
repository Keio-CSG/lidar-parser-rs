use hesai_pcap_parser_rs::{parse_args, run};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let args = parse_args("hesai-pcap-parser-rs", &args[1..].to_vec());
    run(args);
}
