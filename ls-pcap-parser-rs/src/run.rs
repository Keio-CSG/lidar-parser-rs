use std::f32::consts::PI;
use std::fs::File;
use std::path::Path;
use std::time::Instant;

use anyhow::Error;
use byteorder::{BigEndian, ByteOrder};
use pcap_parser::*;
use pcap_parser::traits::PcapReaderIterator;
use writer_common::framewriter::{FrameWriter, CsvWriter, HdfWriter, PcdWriter};
use writer_common::velopoint::VeloPoint;

use crate::signalsplitwriter::SignalSplitWriter;
use crate::{Args, OutType};
use crate::packetinfo::{parse_packet_info, PcapInfo, ReturnMode};
use crate::constants::*;

pub fn run(args: Args) {
    let input_file_path = Path::new(&args.input);
    let stem = input_file_path.file_stem().unwrap();
    let file_dir = input_file_path.parent().unwrap().to_str().unwrap().to_string();

    let dir = stem.to_str().unwrap().to_string();

    let writer_internal: Box<dyn FrameWriter> = match args.out_type {
        OutType::Csv => Box::new(CsvWriter::create(file_dir, dir, stem.to_str().unwrap().to_string())),
        OutType::Hdf => Box::new(HdfWriter::create(file_dir, stem.to_str().unwrap().to_string(), args.compression)),
        OutType::Pcd => Box::new(PcdWriter::create(file_dir, dir, stem.to_str().unwrap().to_string())),
    };
    let mut writer = Box::new(SignalSplitWriter::new(writer_internal));

    let time_start = Instant::now();
    let pcap_info = parse_packet_info(&args.input).unwrap();
    let end = time_start.elapsed();
    println!("{}us", end.as_micros());
    println!("{:?}", pcap_info);

    write_header(&pcap_info, &mut writer);

    let file = File::open(&args.input).unwrap();
    let mut num_packets = 0;
    let mut reader = LegacyPcapReader::new(65536, file).expect("LegacyPcapReader");

    let time_start = Instant::now();
    loop {
        match reader.next() {
            Ok((offset, block)) => {
                match block {
                    PcapBlockOwned::Legacy(packet) => {
                        // println!("{}", packet.data.len());
                        let ether_type = &packet.data[12..14];
                        if ether_type != &[0x08, 0x00] {
                            // not ipv4
                            reader.consume(offset);
                            continue;
                        }
                        
                        let udp_data = &packet.data[42..];
                        if udp_data.is_empty() {
                            reader.consume(offset);
                            continue;
                        }
                        num_packets += 1;

                        let first_byte = udp_data[0];
                        if first_byte < 128 || first_byte == 255 {
                            // data package
                            let factory_return_mode = udp_data[1205];
                            match factory_return_mode {
                                0x01 => parse_body_single(udp_data, &mut writer).expect("parse failed"),
                                0x02 => parse_body_dual(udp_data, &mut writer).expect("parse failed"),
                                _ => (),
                            }
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

fn write_header(info: &PcapInfo, writer: &mut SignalSplitWriter) {
    let laser_num = 128;
    let return_mode = match info.return_mode {
        ReturnMode::Single => 0,
        ReturnMode::Dual => 2,
    };
    let manufacturer = "Leishen";
    let model = "CH128x1";
    writer.write_attribute(laser_num, info.frequency, return_mode, manufacturer, model);
}

/// construction:
/// - point_list (1197 bytes = 171 * 7 bytes):
///   - line_num (1 byte) [0..170]
///   - horizontal angle (2 bytes) [unit: 0.01 degree]
///   - distance (3 bytes) 
///   - strength (1 byte)
/// - additional info (9 bytes):
///   - utc time (3 bytes):
///     - hour (1 byte)
///     - minute (1 byte)
///     - second (1 byte)
///   - timestamp (4 bytes) [unit: us]
///   - vendor (1 byte)
///   - echo (1 byte)
fn parse_body_single(body: &[u8], writer: &mut SignalSplitWriter) -> Result<(), Error> {
    let hour = body[1197];
    let minute = body[1198];
    let second = body[1199];
    let us = BigEndian::read_u32(&body[1200..1204]);
    let timestamp_ns = ((hour as u64 * 60 + minute as u64) * 60 + second as u64) * 1000000000 + us as u64 * 1000;
    for _ in 0..171 {
        let line_num = body[0];
        let horizontal_angle = BigEndian::read_u16(&body[1..3]);
        let distance_cm = BigEndian::read_u16(&body[3..5]) as f32 
            + body[5] as f32 / 256.0;
        let strength = body[6];

        let azimuth = horizontal_angle;
        let altitude = get_altitude(azimuth, line_num);
        let distance_m = distance_cm / 100.0;

        let omega = altitude as f32 * PI / 18000.0;
        let alpha = azimuth as f32 * PI / 18000.0;

        let x = distance_m * omega.cos() * alpha.sin();
        let y = distance_m * omega.cos() * alpha.cos();
        let z = distance_m * omega.sin();

        writer.write_row(VeloPoint {
            intensity: strength,
            channel: line_num,
            azimuth,
            distance_m,
            timestamp: timestamp_ns,
            altitude,
            x, y, z,
        })
    }
    Ok(())
}

/// construction:
/// - point_list (1199 bytes = 109 * 11 bytes):
///   - line_num (1 byte) [0..170]
///   - horizontal angle (2 bytes) [unit: 0.01 degree]
///   - distance1 (3 bytes) 
///   - strength1 (1 byte)
///   - distance2 (3 bytes)
///   - strength2 (1 byte)
/// - additional info (7 bytes):
///   - utc time (1 byte):
///     - second (1 byte)
///   - timestamp (4 bytes) [unit: us]
///   - vendor (1 byte)
///   - echo (1 byte)
fn parse_body_dual(body: &[u8], writer: &mut SignalSplitWriter) -> Result<(), Error> {
    let second = body[1199];
    let us = BigEndian::read_u32(&body[1200..1204]);
    let timestamp_ns = second as u64 * 1000000000 + us as u64 * 1000;

    for i in 0..109 {
        let point = &body[11 * i..11 * (i + 1)];
        let line_num = point[0];
        if line_num == 255 {
            // frame split signal
            writer.split_frame();
            continue;
        }
        let horizontal_angle = BigEndian::read_u16(&point[1..3]);
        for _ in 0..2 {
            let distance_cm = BigEndian::read_u16(&point[3..5]) as f32 
            + point[5] as f32 / 256.0;
            let strength = point[6];

            let azimuth = horizontal_angle;
            let altitude = get_altitude(azimuth, line_num);
            let distance_m = distance_cm / 100.0;

            let omega = altitude as f32 * PI / 18000.0;
            let alpha = azimuth as f32 * PI / 18000.0;

            let x = distance_m * omega.cos() * alpha.sin();
            let y = distance_m * omega.cos() * alpha.cos();
            let z = distance_m * omega.sin();

            writer.write_row(VeloPoint {
                intensity: strength,
                channel: line_num,
                azimuth,
                distance_m,
                timestamp: timestamp_ns,
                altitude,
                x, y, z,
            })
        }
    }
    Ok(())
}

fn get_altitude(azimuth: u16, line_num: u8) -> i16 {
    let azimuth_rad = azimuth as f32 * PI / 18000.0;
    let one_index = (line_num / 4) as usize;
    let two_index = (line_num % 4) as usize;
    let r = COS_THETA_TWO_LIST[two_index] * COS_THETA_ONE_LIST[one_index] * (azimuth_rad / 2.0).sin() - SIN_THETA_TWO_LIST[two_index] * SIN_THETA_ONE_LIST[one_index];
    let sin_theat = SIN_THETA_ONE_LIST[one_index] + 2.0 * r * SIN_THETA_TWO_LIST[two_index];

    (sin_theat.asin() * 18000.0 / PI) as i16
}
