use pcap_parser::*;
use pcap_parser::traits::PcapReaderIterator;
use velopoint::VeloPoint;
use std::fs::File;
use std::path::Path;
use std::process::exit;
use std::time::Instant;
use getopts::Options;
use anyhow::{Result, Error, ensure, anyhow};

use crate::csvwriter::CsvWriter;
use crate::framewriter::FrameWriter;
use crate::hdfwriter::HdfWriter;

mod csvwriter;
mod hdfwriter;
mod velopoint;
mod framewriter;

// TODO: dual returnでreturnが1つしかない場合に対応する

pub fn run(args: Args) {
    let stem = Path::new(&args.input).file_stem().unwrap();

    let dir = format!("{}/", stem.to_str().unwrap());

    let mut writer: Box<dyn FrameWriter> = match args.out_type {
        OutType::Csv => Box::new(CsvWriter::create(dir, stem.to_str().unwrap().to_string())),
        OutType::Hdf => Box::new(HdfWriter::create(stem.to_str().unwrap().to_string(), args.compression)),
    };

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
                        parse_packet_body(udp_data, &pcap_info, &mut writer).expect("parse failed");
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

pub enum OutType {
    Csv,
    Hdf,
}

pub struct Args {
    input: String,
    out_type: OutType,
    compression: bool,
}

pub fn parse_args(args: &Vec<String>) -> Args {
    let mut opts = Options::new();
    opts.optopt("o", "output", "output type", "csv|hdf");
    opts.optflag("h", "help", "print this help menu");
    opts.optflag("c", "compression", "enable compression");
    let matches = opts.parse(args).unwrap();
    if matches.opt_present("h") {
        print!("{}", opts.usage("Usage: veloconv [options] <input>"));
        exit(0);
    }
    let input = if !matches.free.is_empty() {
        matches.free[0].clone()
    } else {
        print!("{}", opts.usage("Usage: veloconv [options] <input>"));
        exit(0);
    };
    let out_type = if matches.opt_present("o") {
        match matches.opt_str("o").unwrap().as_str() {
            "csv" => OutType::Csv,
            "hdf" => OutType::Hdf,
            _ => {
                print!("{}", opts.usage("Usage: veloconv [options] <input>"));
                exit(0);
            }
        }
    } else {
        OutType::Csv
    };
    let compression = matches.opt_present("c");
    Args { input, out_type, compression }
}

fn write_header(info: &PcapInfo, writer: &mut Box<dyn FrameWriter>) {
    let laser_num = match info.product {
        VeloProduct::Vlp16 => 16,
        VeloProduct::Vlp32c => 32,
    };
    let return_mode = match info.return_mode {
        ReturnMode::Strongest => 0,
        ReturnMode::Last => 1,
        ReturnMode::Dual => 2,
    };
    let manufacturer = "Velodyne";
    let model = match info.product {
        VeloProduct::Vlp16 => "VLP-16",
        VeloProduct::Vlp32c => "VLP-32C",
    };
    writer.write_attribute(laser_num, info.motor_speed as u32, return_mode, manufacturer, model);
}

fn parse_packet_body(packet_body: &[u8], info: &PcapInfo, writer: &mut Box<dyn FrameWriter>) -> Result<(), Error> {
    ensure!(packet_body.len() == 1206, "packet size is not 1206");
    let timestamp = ((packet_body[1200] as u32) << 24) + ((packet_body[1201] as u32) << 16) + ((packet_body[1202] as u32) << 8) + packet_body[1203] as u32;

    let blocks = &packet_body[0..1200];

    let azimuth_per_scan = (info.motor_speed as f32 * 36000.0 / 60.0 / 1000000.0 * 55.296).round() as u16;

    match info.product {
        VeloProduct::Vlp16 => {
            parse_vlp16(blocks, &info.return_mode, azimuth_per_scan, timestamp, writer)?;
        },
        VeloProduct::Vlp32c => {
            parse_vlp32c(blocks, &info.return_mode, azimuth_per_scan, timestamp, writer)?;
        },
    }

    Ok(())
}

const VLP16_LASER_ANGLES: [f32; 16] = [
    -15.0, 1.0, -13.0, 3.0, -11.0, 5.0, -9.0, 7.0, -7.0, 9.0, -5.0, 11.0, -3.0, 13.0, -1.0, 15.0,
];
const VLP16_DISTANCE_RESOLUTION: f32 = 0.002;
fn parse_vlp16(blocks: &[u8], return_mode: &ReturnMode, azimuth_per_scan: u16, timestamp: u32, writer: &mut Box<dyn FrameWriter>) -> Result<(), Error> {
    // blocks: 100 bytes * 12
    //   flag(0xFFEE)  : 2 bytes
    //   azimuth       : 2 bytes
    //   channel data A: 3 bytes * 16
    //   channel data B: 3 bytes * 16
    //     distance    : 2 bytes
    //     reflectivity: 1 byte
    
    for i in 0..12 {
        let block = &blocks[i*100..(i+1)*100];
        let flag = ((block[0] as u16) << 8) + block[1] as u16;
        ensure!(flag == 0xFFEE, "block flag is not 0xFFEE");
        let block_azimuth = ((block[3] as u16) << 8) + block[2] as u16;

        for step in 0..=1 {
            let step_start_offset = (4 + step * 48) as usize;
            let azimuth = block_azimuth + step * azimuth_per_scan;
            let azimuth = if azimuth > 36000 { azimuth - 36000 } else { azimuth };

            for channel in 0..16 {
                // calculate precise azimuth
                let precise_azimuth = azimuth + channel * azimuth_per_scan / 24;
                let precise_azimuth = if precise_azimuth > 36000 { precise_azimuth - 36000 } else { precise_azimuth };
                
                // calculate precise timestamp
                let full_firing_cycle = 55.296;
                let single_firing = 2.304;
                let x = i as u16;
                let y = step * 16 + channel;
                let data_block_index = match return_mode {
                    ReturnMode::Dual => (x - (x % 2)) + (y / 16),
                    _ => (x * 2) + (y / 16),
                };
                let data_point_index = channel;
                let timing_offset = full_firing_cycle * data_block_index as f32 + single_firing * data_point_index as f32;
                let precise_timestamp = timestamp as f32 + timing_offset;
                
                let channel_start = step_start_offset + (channel * 3) as usize;
                let channel_end = step_start_offset + ((channel + 1) * 3) as usize;
                let channel_data = &block[channel_start..channel_end];

                let distance = ((channel_data[1] as u16) << 8) + channel_data[0] as u16;
                let reflectivity = channel_data[2];
                let point = build_velo_point(distance as f32, precise_azimuth, channel as u16, (precise_timestamp * 1000.0) as u32, reflectivity as u16, &VLP16_LASER_ANGLES, VLP16_DISTANCE_RESOLUTION);
                writer.write_row(point);
            }
        }
    }
    Ok(())
}

const VLP32C_LASER_ANGLES: [f32; 32] = [
    -25.0 , -1.0 , -1.667, -15.639, -11.31, 0.0  , -0.667, -8.843, 
    -7.254, 0.333, -0.333, -6.148 , -5.333, 1.333, 0.667 , -4.0  ,
    -4.667, 1.667, 1.0   , -3.667 , -3.333, 3.333, 2.333 , -2.667,
    -3.0  , 7.0  , 4.667 , -2.333 , -2.0  , 15.0 , 10.333, -1.333
];
const VLP32C_AZIMUTH_OFFSETS: [i32; 32] = [
    140, -420,  140, -140,  140, -140,  420, -140,
    140, -420,  140, -140,  420, -140,  420, -140,
    140, -420,  140, -420,  420, -140,  140, -140,
    140, -140,  140, -420,  420, -140,  140, -140
];
const VLP32C_DISTANCE_RESOLUTION: f32 = 0.004;
fn parse_vlp32c(blocks: &[u8], return_mode: &ReturnMode, azimuth_per_scan: u16, timestamp: u32, writer: &mut Box<dyn FrameWriter>) -> Result<(), Error> {
    // blocks: 100 bytes * 12
    //   flag(0xFFEE)  : 2 bytes
    //   azimuth       : 2 bytes
    //   channel data  : 3 bytes * 32
    //     distance    : 2 bytes
    //     reflectivity: 1 byte
    
    for i in 0..12 {
        let block = &blocks[i*100..(i+1)*100];
        let flag = ((block[0] as u16) << 8) + block[1] as u16;
        ensure!(flag == 0xFFEE, "block flag is not 0xFFEE");
        let block_azimuth = ((block[3] as u16) << 8) + block[2] as u16;

        for channel in 0..32 {
            // calculate precise azimuth
            let precise_azimuth = (block_azimuth + channel * azimuth_per_scan / 24) as i32 + VLP32C_AZIMUTH_OFFSETS[channel as usize];
            let precise_azimuth = (precise_azimuth % 36000) as u16;
            
            // calculate precise timestamp
            let full_firing_cycle = 55.296;
            let single_firing = 2.304;
            let x = i as u16;
            let y = channel;
            let data_block_index = match return_mode {
                ReturnMode::Dual => x / 2,
                _ => x,
            };
            let data_point_index = y / 2;
            let timing_offset = full_firing_cycle * data_block_index as f32 + single_firing * data_point_index as f32;
            let precise_timestamp = timestamp as f32 + timing_offset;
            
            let channel_start = (channel * 3) as usize;
            let channel_end = ((channel + 1) * 3) as usize;
            let channel_data = &block[channel_start..channel_end];

            let distance = ((channel_data[1] as u16) << 8) + channel_data[0] as u16;
            let reflectivity = channel_data[2];
            let point = build_velo_point(distance as f32, precise_azimuth, channel as u16, (precise_timestamp * 1000.0) as u32, reflectivity as u16, &VLP32C_LASER_ANGLES, VLP32C_DISTANCE_RESOLUTION);
            writer.write_row(point);
        }
    }
    Ok(())
}


const ROTATION_RESOLUTION: f32 = 0.01;
fn build_velo_point(distance: f32, azimuth: u16, channel: u16, timestamp_ns: u32, reflectivity: u16, laser_angles: &[f32], distance_resolution: f32) -> VeloPoint {
    let distance_m = distance as f32 * distance_resolution;
    let vertical_angle = laser_angles[channel as usize];
    let omega = vertical_angle.to_radians();
    let alpha = (azimuth as f32 * ROTATION_RESOLUTION).to_radians();

    let x = distance_m * omega.cos() * alpha.sin();
    let y = distance_m * omega.cos() * alpha.cos();
    let z = distance_m * omega.sin();

    VeloPoint {
        reflectivity: reflectivity as u8,
        channel: channel as u8,
        azimuth,
        distance_m,
        timestamp: timestamp_ns,
        vertical_angle,
        x,
        y,
        z,
    }
}

#[derive(Debug)]
enum ReturnMode {
    Strongest,
    Last,
    Dual,
}


#[derive(Debug)]
enum VeloProduct {
    Vlp16,
    Vlp32c,
}

#[derive(Debug)]
struct PcapInfo {
    return_mode: ReturnMode,
    product: VeloProduct,
    num_frames: u16,
    motor_speed: u16, // rpm
}

fn parse_packet_info(filename: &str) -> Result<PcapInfo, Error> {
    let file = File::open(filename).unwrap();
    let mut reader = LegacyPcapReader::new(65536, file).expect("LegacyPcapReader");

    let mut packet_body: Option<Vec<u8>> = None;
    let mut num_frames: u16 = 1;
    let mut prev_azimuth: u16 = 0;

    loop {
        match reader.next() {
            Ok((offset, block)) => {
                match block {
                    PcapBlockOwned::Legacy(packet) => {
                        // etherのヘッダ長は14byte
                        let ether_data = &packet.data[14..];
                        // ipv4のヘッダ長は可変(基本20byte)
                        let ip_header_size = ((ether_data[0] & 15) * 4) as usize;
                        let packet_size = (((ether_data[2] as u32) << 8) + ether_data[3] as u32) as usize;
                        let ip_data = &ether_data[ip_header_size..packet_size];
                        // udpのヘッダ長は8byte
                        let udp_data = &ip_data[8..];
                        
                        // 最初のblockのazimuthを見て、フレーム数をカウント
                        let first_block_azimuth = ((udp_data[3] as u16) << 8) + udp_data[2] as u16;
                        if first_block_azimuth < prev_azimuth {
                            num_frames += 1;
                        }
                        prev_azimuth = first_block_azimuth;
                        if packet_body.is_none() {
                            packet_body = Some(udp_data.to_vec());
                        }
                    }
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

    let packet_body = match packet_body {
        Some(body) => body,
        None => return Err(anyhow!("no packet found")),
    };

    ensure!(packet_body.len() == 1206, "packet size is not 1206");
    let factory_return_mode = packet_body[1204];
    let factory_product_id = packet_body[1205];

    let return_mode = match factory_return_mode {
        0x37 => ReturnMode::Strongest,
        0x38 => ReturnMode::Last,
        0x39 => ReturnMode::Dual,
        _ => return Err(anyhow!("unknown return mode: {}", factory_return_mode)),
    };

    let product = match factory_product_id {
        0x22 => VeloProduct::Vlp16,
        0x28 => VeloProduct::Vlp32c,
        _ => return Err(anyhow!("unknown product id: {}", factory_product_id)),
    };

    // predict motor speed
    let first_block_azimuth = ((packet_body[3] as u16) << 8) + packet_body[2] as u16;
    let last_block_azimuth = ((packet_body[1103] as u16) << 8) + packet_body[1102] as u16;
    let azimuth_diff = if first_block_azimuth > last_block_azimuth {
        36000 + last_block_azimuth - first_block_azimuth
    } else {
        last_block_azimuth - first_block_azimuth
    };
    let elapsed_time_us = match return_mode {
        ReturnMode::Dual => 55.296 * 11.0,
        _ => 55.296 * 22.0,
    };
    let motor_speed = (azimuth_diff as f32 / elapsed_time_us * 10000.0 / 6.0) as u16;

    Ok(PcapInfo {
        return_mode,
        product,
        num_frames,
        motor_speed,
    })
}
