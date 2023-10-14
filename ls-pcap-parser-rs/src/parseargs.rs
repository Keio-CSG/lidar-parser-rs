use std::process::exit;

use getopts::Options;

pub enum OutType {
    Csv,
    Hdf
}

pub struct Args {
    pub(crate) input: String,
    pub(crate) out_type: OutType,
    #[allow(dead_code)]
    pub(crate) frame_time_ms: u64,
    pub(crate) compression: bool,
}

pub fn parse_args(command_prefix: &str, args: &Vec<String>) -> Args {
    let mut opts = Options::new();
    opts.optopt("o", "output", "output type", "csv|hdf");
    opts.optopt("t", "frame-time", "frame time in milliseconds", "100");
    opts.optflag("h", "help", "print this help menu");
    opts.optflag("c", "compression", "enable compression");
    let matches = opts.parse(args).unwrap();
    if matches.opt_present("h") {
        print_help(opts, command_prefix);
        exit(0);
    }
    let input = if !matches.free.is_empty() {
        matches.free[0].clone()
    } else {
        print_help(opts, command_prefix);
        exit(0);
    };
    let out_type = if matches.opt_present("o") {
        match matches.opt_str("o").unwrap().as_str() {
            "csv" => OutType::Csv,
            "hdf" => OutType::Hdf,
            _ => {
                print_help(opts, command_prefix);
                exit(0);
            }
        }
    } else {
        OutType::Csv
    };
    let frame_time_ms = if matches.opt_present("t") {
        match matches.opt_str("t").unwrap().parse::<u64>() {
            Ok(v) => v,
            Err(_) => {
                print_help(opts, command_prefix);
                exit(0);
            }
        }
    } else {
        100
    };
    let compression = matches.opt_present("c");
    Args {
        input,
        out_type,
        frame_time_ms,
        compression,
    }
}

fn print_help(opts: Options, command_prefix: &str) {
    print!("{}", opts.usage(format!("Usage: {} [options] <input>", command_prefix).as_str()));
}
