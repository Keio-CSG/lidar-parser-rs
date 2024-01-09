use std::{path::{Path, PathBuf}, fs::File, io::Read, time::Instant};

use pcap_parser::{LegacyPcapReader, traits::PcapReaderIterator, PcapBlockOwned, PcapError};
use writer_common::{framewriter::{FrameWriter, CsvWriter, HdfWriter, PcdWriter}, timesplitwriter::TimeSplitWriter};

use crate::{parseargs::{Args, OutType}, parser::{lvx::parse_lvx, lvx2::parse_lvx2, pcap::{parse_packet_body, write_header}}};

pub fn run(args: Args) {
    let input_file_path = Path::new(&args.input);
    let stem = input_file_path.file_stem().unwrap();
    let mut file_dir = input_file_path.parent().unwrap().to_str().unwrap().to_string();
    if file_dir == "" {
        file_dir = ".".to_string();
    }

    let dir = stem.to_str().unwrap().to_string();

    let writer_internal: Box<dyn FrameWriter> = match args.out_type {
        OutType::Csv => Box::new(CsvWriter::create(file_dir, dir, stem.to_str().unwrap().to_string())),
        OutType::Hdf => Box::new(HdfWriter::create(file_dir, stem.to_str().unwrap().to_string(), args.compression)),
        OutType::Pcd => Box::new(PcdWriter::create(file_dir, dir, stem.to_str().unwrap().to_string())),
    };
    let mut writer = TimeSplitWriter::new(writer_internal, args.frame_time_ms * 1000 * 1000, 0);

    let file_path = PathBuf::from(&args.input);
    let extension = file_path.extension().unwrap().to_str().unwrap();
    match extension {
        "pcap" => {
            read_pcap_file(&args.input, args.frame_time_ms, &mut writer);
        }
        "lvx" | "lvx2" => {
            read_lvx_file(&args.input, args.frame_time_ms, &mut writer);
        }
        _ => {
            eprintln!("Invalid file format");
            std::process::exit(1);
        }
    }
}

fn read_pcap_file(path: &str, frame_time_ms: u64, mut writer: &mut TimeSplitWriter) {
    let file = File::open(path).unwrap();
    let mut num_packets = 0;
    let mut reader = LegacyPcapReader::new(65536, file).expect("LegacyPcapReader");

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
                        // if udp_data[0] != 0xEE {
                        //     // ignore non-lidar packets
                        //     reader.consume(offset);
                        //     continue;
                        // }
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
            Err(PcapError::Eof) => {
                writer.finalize();
                break;
            },
            Err(PcapError::Incomplete) => {
                reader.refill().unwrap();
            },
            Err(e) => panic!("error while reading: {:?}", e),
        }
    }
    let duration = time_start.elapsed();

    println!("{} packets have been processed in {:?}", num_packets, duration);
}

fn read_lvx_file(path: &str, frame_time_ms: u64, mut writer: &mut TimeSplitWriter) {
    let file = File::open(path).unwrap();
    let mut reader = std::io::BufReader::new(file);

    let time_start = Instant::now();

    // check public header
    let mut public_header_block = [0u8; 24];
    reader.read_exact(&mut public_header_block).unwrap();
    if public_header_block[..10] != "livox_tech".as_bytes()[..] // file signature
    || public_header_block[20..24] != [0x67, 0xA7, 0x0E, 0xAC] {// magic code
        eprintln!("Invalid file format");
        std::process::exit(1);
    }
    let ver_a = public_header_block[16];
    let ver_b = public_header_block[17];
    let ver_c = public_header_block[18];
    let ver_d = public_header_block[19];
    match (ver_a, ver_b, ver_c, ver_d) {
        (1, 1, 0, 0) => {
            parse_lvx(&mut reader, frame_time_ms, &mut writer).unwrap();
        },
        (2, 0, 0, 0) => {
            parse_lvx2(&mut reader, frame_time_ms, &mut writer).unwrap();
        }
        _ => {
            eprintln!("Invalid file format version: {}.{}.{}.{}", ver_a, ver_b, ver_c, ver_d);
            std::process::exit(1);
        }
    }
    writer.finalize();
    let duration = time_start.elapsed();

    println!("file have been processed in {:?}", duration);
}
