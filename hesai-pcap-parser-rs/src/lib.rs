use pcap_parser::*;
use pcap_parser::traits::PcapReaderIterator;
use writer_common::azimuthsplitwriter::AzimuthSplitWriter;
use writer_common::framewriter::{FrameWriter, CsvWriter, HdfWriter};
use writer_common::velopoint::VeloPoint;
use std::fs::File;
use std::f32::consts::PI;
use std::path::Path;
use std::process::exit;
use std::time::Instant;
use getopts::Options;

pub fn run(args: Args) {
    let stem = Path::new(&args.input).file_stem().unwrap();

    //let start = Instant::now();
    let file = File::open(&args.input).unwrap();
    let mut num_packets = 0;
    let mut reader = LegacyPcapReader::new(65536, file).expect("LegacyPcapReader");

    let dir = format!("{}/", stem.to_str().unwrap());

    let writer_internal: Box<dyn FrameWriter> = match args.out_type {
        OutType::Csv => Box::new(CsvWriter::create(dir, stem.to_str().unwrap().to_string())),
        OutType::Hdf => Box::new(HdfWriter::create(stem.to_str().unwrap().to_string(), args.compression)),
    };
    let mut writer = Box::new(AzimuthSplitWriter::new(writer_internal));

    let mut header_written = false;

    let time_start = Instant::now();
    loop {
        match reader.next() {
            Ok((offset, block)) => {
                num_packets += 1;
                match block {
                    PcapBlockOwned::Legacy(packet) => {
                        // println!("{}", packet.data.len());
                        // etherのヘッダ長は14byte
                        let ether_data = &packet.data[14..];
                        // ipv4のヘッダ長は可変(基本20byte)
                        let ip_header_size = ((ether_data[0] & 15) * 4) as usize;
                        let packet_size = (((ether_data[2] as u32) << 8) + ether_data[3] as u32) as usize;
                        let ip_data = &ether_data[ip_header_size..packet_size];
                        // udpのヘッダ長は8byte
                        let udp_data = &ip_data[8..ip_data.len()];
                        parse_packet_body(udp_data, &mut writer);
                        if !header_written {
                            header_written = true;
                            write_header(udp_data, &mut writer);
                        }
                    },
                    _ => ()
                }
                reader.consume(offset);
            },
            Err(PcapError::Eof) => break,
            Err(PcapError::Incomplete) => {
                reader.refill().unwrap();
            },
            Err(e) => panic!("error while reading: {:?}", e),
        }
    }
    let duration = time_start.elapsed();

    println!("{} packets have been processed in {:?}", num_packets, duration);
    //let end = start.elapsed();
    //println!("{}.{:03}sec", end.as_secs(), end.subsec_millis() / 1000)
}

pub enum OutType {
    Csv,
    Hdf
}

pub struct Args {
    input: String,
    out_type: OutType,
    compression: bool,
}

pub fn parse_args(command_prefix: &str, args: &Vec<String>) -> Args {
    let mut opts = Options::new();
    opts.optopt("o", "output", "output type", "csv|hdf");
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
    let compression = matches.opt_present("c");
    Args { input, out_type, compression }
}

fn print_help(opts: Options, command_prefix: &str) {
    print!("{}", opts.usage(format!("Usage: {} [options] <input>", command_prefix).as_str()));
}

fn write_header(packet_body: &[u8], writer: &mut AzimuthSplitWriter) {
    let header = &packet_body[6..12];
    let laser_num = header[0] as u32;
    let tail = &packet_body[1052..1076];
    let return_mode = match tail[10] {
        0x37 => 0, // Strongest
        0x38 => 1, // Last
        0x39 => 2, // Dual
        _ => 0,
    };
    let motor_speed = ((tail[12] as u32) << 8) + (tail[11] as u32);
    let frequency = motor_speed as f32 / 60.0;
    writer.write_attribute(laser_num, frequency, return_mode, "Hesai", "XT32");
}

fn parse_packet_body(packet_body: &[u8], writer: &mut AzimuthSplitWriter) {
    // let pre_header = &packet_body[0..6];
    let header = &packet_body[6..12];
    let block_num = header[1] as u32;

    let body = &packet_body[12..1052];
    
    let tail = &packet_body[1052..1076];
    let return_mode = tail[10];
    let date_time = &tail[13..19];
    let timestamp_us = ((tail[22] as u32) << 24) 
                        + ((tail[21] as u32) << 16) 
                        + ((tail[20] as u32) << 8) 
                        + ((tail[19] as u32));

    for block_index in 0..block_num {
        let block_timestamp_ns = calc_block_timestamp_ns(date_time, timestamp_us, block_index+1, return_mode);
        let block_start = (block_index*130) as usize;
        parse_block(&body[block_start..block_start+130], block_timestamp_ns, writer);
    }
}

fn calc_block_timestamp_ns(date_time: &[u8], timestamp_us: u32, block_id: u32, return_mode: u8) -> u64 {
    let t0 = (date_time[4] as u64) * 60 * 1000000000 + (date_time[5] as u64) * 1000000000 + timestamp_us as u64 * 1000;
    if return_mode == 0x37 || return_mode == 0x38 {
        t0 + 3280 - 50000 * (8 - block_id as u64)
    }
    else {
        t0 + 3280 - (50000 * ((8 - block_id as u64)/2))
    }
}

fn channel_to_v_angle(channel: i32) -> i32 {
    -channel + 16
}

fn calc_polar_coordinate(azimuth_deg: f32, v_angle_deg: f32, distance_m: f32) -> (f32,f32,f32) {
    let azimuth_rad = azimuth_deg * PI / 180.0;
    let v_angle_rad = v_angle_deg * PI / 180.0;
    let x = distance_m * v_angle_rad.cos() * azimuth_rad.sin();
    let y = distance_m * v_angle_rad.cos() * azimuth_rad.cos();
    let z = distance_m * v_angle_rad.sin();
    (x,y,z)
}

fn parse_block(packet_block: &[u8], block_timestamp_ns: u64, writer: &mut AzimuthSplitWriter) {
    let azimuth = ((packet_block[1] as u32) << 8) + (packet_block[0] as u32);
    for channel in 0..32 as u8 {
        let channel_timestamp_ns = block_timestamp_ns + 1512 * channel as u64 + 280;
        let v_angle = channel_to_v_angle(channel as i32);
        let channel_start = (2+channel*4) as usize;
        let channel_data = &packet_block[channel_start..channel_start+4];
        let distance = ((channel_data[1] as u32) << 8) + (channel_data[0] as u32);
        let reflectivity = channel_data[2];
        let (x,y,z) = calc_polar_coordinate(
            azimuth as f32 / 100.0, 
            v_angle as f32, 
            distance as f32 * 4.0 / 1000.0);

        writer.write_row(VeloPoint { 
            intensity: reflectivity, 
            channel, 
            azimuth: azimuth as u16, 
            distance_m: distance as f32 * 4.0 / 1000.0,
            timestamp: channel_timestamp_ns, 
            altitude: (v_angle * 100) as i16, 
            x, y, z })
    }
}
