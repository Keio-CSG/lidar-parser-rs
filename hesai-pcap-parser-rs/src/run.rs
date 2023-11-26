use std::{path::Path, fs::File, time::Instant};

use pcap_parser::{LegacyPcapReader, traits::PcapReaderIterator, PcapBlockOwned, PcapError};
use writer_common::{framewriter::{FrameWriter, CsvWriter, HdfWriter, PcdWriter}, azimuthsplitwriter::AzimuthSplitWriter};

use crate::{Args, OutType, detect_model::{detect_model, HesaiModel}, parse_xt32::{parse_packet_body_xt32, write_header_xt32}, parse_at128::{parse_packet_body_at128, write_header_at128}};

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
        OutType::Pcd => Box::new(PcdWriter::create(dir, stem.to_str().unwrap().to_string())),
    };
    let mut writer = Box::new(AzimuthSplitWriter::new_with_min_offset(writer_internal, 60*100));

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
                        if udp_data[0] != 0xEE {
                            // ignore non-lidar packets
                            reader.consume(offset);
                            continue;
                        }
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

fn write_header(packet_body: &[u8], writer: &mut AzimuthSplitWriter) {
    let pre_header = &packet_body[0..6];
    let model = detect_model(pre_header);
    match model {
        HesaiModel::XT32 => write_header_xt32(packet_body, writer),
        HesaiModel::AT128 => write_header_at128(packet_body, writer),
        _ => panic!("Unknown model"),
    }
}

fn parse_packet_body(packet_body: &[u8], writer: &mut AzimuthSplitWriter) {
    let pre_header = &packet_body[0..6];
    let model = detect_model(pre_header);
    match model {
        HesaiModel::XT32 => parse_packet_body_xt32(packet_body, writer),
        HesaiModel::AT128 => parse_packet_body_at128(packet_body, writer),
        _ => panic!("Unknown model"),
    }
}
