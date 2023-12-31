use pcap_parser::*;
use pcap_parser::traits::PcapReaderIterator;
use writer_common::{framewriter::{FrameWriter, CsvWriter, HdfWriter, PcdWriter}, velopoint::VeloPoint, valueslopesplitwriter::ValueSlopeSplitWriter};
use std::fs::File;
use std::path::Path;
use std::process::exit;
use std::time::Instant;
use getopts::Options;
use anyhow::{Result, Error, ensure, anyhow};
use byteorder::{LittleEndian, ByteOrder};

// TODO: dual returnでreturnが1つしかない場合に対応する

pub fn run(args: Args) {
    let input_file_path = Path::new(&args.input);
    let stem = input_file_path.file_stem().unwrap();
    let mut file_dir = input_file_path.parent().unwrap().to_str().unwrap().to_string();
    if file_dir == "" {
        file_dir = ".".to_string();
    }

    let dir = stem.to_str().unwrap().to_string();

    let pcap_info = parse_packet_info(&args.input).unwrap();

    let writer_internal: Box<dyn FrameWriter> = match args.out_type {
        OutType::Csv => Box::new(CsvWriter::create(file_dir, dir, stem.to_str().unwrap().to_string())),
        OutType::Hdf => Box::new(HdfWriter::create(file_dir, stem.to_str().unwrap().to_string(), args.compression)),
        OutType::Pcd => Box::new(PcdWriter::create(file_dir, dir, stem.to_str().unwrap().to_string())),
    };
    let mut writer = Box::new(ValueSlopeSplitWriter::new(writer_internal, pcap_info.num_frames as u64));

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
    Pcd,
}

pub struct Args {
    input: String,
    out_type: OutType,
    compression: bool,
}

pub fn parse_args(command_prefix: &str, args: &Vec<String>) -> Args {
    let mut opts = Options::new();
    opts.optopt("o", "output", "output type", "csv|hdf|pcd");
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
            "pcd" => OutType::Pcd,
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

fn write_header(info: &PcapInfo, writer: &mut ValueSlopeSplitWriter) {
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
    writer.write_attribute(laser_num, info.frequency, return_mode, manufacturer, model);
}

fn parse_packet_body(packet_body: &[u8], info: &PcapInfo, writer: &mut ValueSlopeSplitWriter) -> Result<(), Error> {
    ensure!(packet_body.len() == 1206, "packet size is not 1206");
    let timestamp = LittleEndian::read_u32(&packet_body[1200..1204]);

    let blocks = &packet_body[0..1200];

    let azimuth_per_scan = (info.frequency * 36000.0 / 1000000.0 * 55.296).round() as u16;

    match info.product {
        VeloProduct::Vlp16 => {
            match info.return_mode {
                ReturnMode::Strongest | ReturnMode::Last => {
                    parse_vlp16_single(blocks, azimuth_per_scan, timestamp, writer)?;
                },
                ReturnMode::Dual => {
                    parse_vlp16_dual(blocks, azimuth_per_scan, timestamp, writer)?;
                },
            }
        },
        VeloProduct::Vlp32c => {
            match info.return_mode {
                ReturnMode::Strongest | ReturnMode::Last => {
                    parse_vlp32c_single(blocks, azimuth_per_scan, timestamp, writer)?;
                },
                ReturnMode::Dual => {
                    parse_vlp32c_dual(blocks, azimuth_per_scan, timestamp, writer)?;
                },
            }
        },
    }

    Ok(())
}

const VLP16_LASER_ANGLES: [f32; 16] = [
    -15.0, 1.0, -13.0, 3.0, -11.0, 5.0, -9.0, 7.0, -7.0, 9.0, -5.0, 11.0, -3.0, 13.0, -1.0, 15.0,
];
const VLP16_DISTANCE_RESOLUTION: f32 = 0.002;
fn parse_vlp16_single(blocks: &[u8], azimuth_per_scan: u16, timestamp: u32, writer: &mut ValueSlopeSplitWriter) -> Result<(), Error> {
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
                let data_block_index = (x * 2) + (y / 16);
                let data_point_index = channel;
                let timing_offset = full_firing_cycle * data_block_index as f64 + single_firing * data_point_index as f64;
                let precise_timestamp = timestamp as f64 + timing_offset;
                
                let channel_start = step_start_offset + (channel * 3) as usize;
                let channel_end = step_start_offset + ((channel + 1) * 3) as usize;
                let channel_data = &block[channel_start..channel_end];

                let distance = ((channel_data[1] as u16) << 8) + channel_data[0] as u16;
                let reflectivity = channel_data[2];
                let point = build_velo_point(distance as f32, precise_azimuth, channel as u8, (precise_timestamp * 1000.0) as u64, reflectivity, &VLP16_LASER_ANGLES, VLP16_DISTANCE_RESOLUTION);
                writer.write_row(point, block_azimuth as i64);
            }
        }
    }
    Ok(())
}

fn parse_vlp16_dual(blocks: &[u8], azimuth_per_scan: u16, timestamp: u32, writer: &mut ValueSlopeSplitWriter) -> Result<(), Error> {
    // blocks: 100 bytes * 12
    //   flag(0xFFEE)  : 2 bytes
    //   azimuth       : 2 bytes
    //   channel data A: 3 bytes * 16
    //   channel data B: 3 bytes * 16
    //     distance    : 2 bytes
    //     reflectivity: 1 byte
    
    for i in (0..12).step_by(2) {
        let block_1 = &blocks[i*100..(i+1)*100];
        let block_2 = &blocks[(i+1)*100..(i+2)*100];
        let flag = ((block_1[0] as u16) << 8) + block_1[1] as u16;
        ensure!(flag == 0xFFEE, "block flag is not 0xFFEE");
        let block_azimuth = ((block_1[3] as u16) << 8) + block_1[2] as u16;

        for step in 0..=1 {
            let step_start_offset = (4 + step * 48) as usize;
            let azimuth = block_azimuth + step * azimuth_per_scan;
            let azimuth = if azimuth > 36000 { azimuth - 36000 } else { azimuth };

            for channel in 0..16 {
                let points = (&[block_1, block_2]).map(|block| {
                    // calculate precise azimuth
                    let precise_azimuth = azimuth + channel * azimuth_per_scan / 24;
                    let precise_azimuth = if precise_azimuth > 36000 { precise_azimuth - 36000 } else { precise_azimuth };
                    
                    // calculate precise timestamp
                    let full_firing_cycle = 55.296;
                    let single_firing = 2.304;
                    let x = i as u16;
                    let y = step * 16 + channel;
                    let data_block_index = (x - (x % 2)) + (y / 16);
                    let data_point_index = channel;
                    let timing_offset = full_firing_cycle * data_block_index as f64 + single_firing * data_point_index as f64;
                    let precise_timestamp = timestamp as f64 + timing_offset;
                    
                    let channel_start = step_start_offset + (channel * 3) as usize;
                    let channel_end = step_start_offset + ((channel + 1) * 3) as usize;
                    let channel_data = &block[channel_start..channel_end];

                    let distance = ((channel_data[1] as u16) << 8) + channel_data[0] as u16;
                    let reflectivity = channel_data[2];
                    let point = build_velo_point(distance as f32, precise_azimuth, channel as u8, (precise_timestamp * 1000.0) as u64, reflectivity, &VLP16_LASER_ANGLES, VLP16_DISTANCE_RESOLUTION);
                    point
                });
                if points[0].distance_m == points[1].distance_m {
                    // 同じ点の場合、後の点を無視する
                    writer.write_row(points[0].clone(), block_azimuth as i64);
                } else {
                    writer.write_row(points[0].clone(), block_azimuth as i64);
                    writer.write_row(points[1].clone(), block_azimuth as i64);
                }
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
fn parse_vlp32c_single(blocks: &[u8], azimuth_per_scan: u16, timestamp: u32, writer: &mut ValueSlopeSplitWriter) -> Result<(), Error> {
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
            let precise_azimuth = (block_azimuth + channel / 2 * azimuth_per_scan / 24) as i32 + VLP32C_AZIMUTH_OFFSETS[channel as usize];
            let precise_azimuth = precise_azimuth % 36000;
            let precise_azimuth = if precise_azimuth < 0 { 
                (precise_azimuth + 36000) as u16 
            } else { 
                precise_azimuth as u16 
            };
            
            // calculate precise timestamp
            let full_firing_cycle = 55.296;
            let single_firing = 2.304;
            let x = i as u16;
            let y = channel;
            let data_block_index = x;
            let data_point_index = y / 2;
            let timing_offset = full_firing_cycle * data_block_index as f64 + single_firing * data_point_index as f64;
            let precise_timestamp = timestamp as f64 + timing_offset;
            
            let channel_start = 4 + (channel * 3) as usize;
            let channel_end = 4 + ((channel + 1) * 3) as usize;
            let channel_data = &block[channel_start..channel_end];

            let distance = LittleEndian::read_u16(&channel_data[0..2]);
            let reflectivity = channel_data[2];
            let point = build_velo_point(distance as f32, precise_azimuth, channel as u8, (precise_timestamp * 1000.0) as u64, reflectivity, &VLP32C_LASER_ANGLES, VLP32C_DISTANCE_RESOLUTION);
            writer.write_row(point, block_azimuth as i64);
        }
    }
    Ok(())
}

fn parse_vlp32c_dual(blocks: &[u8], azimuth_per_scan: u16, timestamp: u32, writer: &mut ValueSlopeSplitWriter) -> Result<(), Error> {
    // blocks: 100 bytes * 12
    //   flag(0xFFEE)  : 2 bytes
    //   azimuth       : 2 bytes
    //   channel data  : 3 bytes * 32
    //     distance    : 2 bytes
    //     reflectivity: 1 byte
    
    for i in (0..12).step_by(2) {
        let block_1 = &blocks[i*100..(i+1)*100];
        let block_2 = &blocks[(i+1)*100..(i+2)*100];
        let flag = ((block_1[0] as u16) << 8) + block_2[1] as u16;
        ensure!(flag == 0xFFEE, "block flag is not 0xFFEE");
        let block_azimuth = LittleEndian::read_u16(&block_1[2..4]);

        for channel in 0..32 {
            let points = (&[block_1, block_2]).map(|block| {
                // calculate precise azimuth
                let precise_azimuth = (block_azimuth + channel / 2 * azimuth_per_scan / 24) as i32 + VLP32C_AZIMUTH_OFFSETS[channel as usize];
                let precise_azimuth = precise_azimuth % 36000;
                let precise_azimuth = if precise_azimuth < 0 { 
                    (precise_azimuth + 36000) as u16 
                } else { 
                    precise_azimuth as u16 
                };
                
                // calculate precise timestamp
                let full_firing_cycle = 55.296;
                let single_firing = 2.304;
                let x = i as u16;
                let y = channel;
                let data_block_index = x / 2;
                let data_point_index = y / 2;
                let timing_offset = full_firing_cycle * data_block_index as f64 + single_firing * data_point_index as f64;
                let precise_timestamp = timestamp as f64 + timing_offset;
                
                let channel_start = 4 + (channel * 3) as usize;
                let channel_end = 4 + ((channel + 1) * 3) as usize;
                let channel_data = &block[channel_start..channel_end];
                
                let distance = LittleEndian::read_u16(&channel_data[0..2]);
                let reflectivity = channel_data[2];
                let point = build_velo_point(distance as f32, precise_azimuth, channel as u8, (precise_timestamp * 1000.0) as u64, reflectivity, &VLP32C_LASER_ANGLES, VLP32C_DISTANCE_RESOLUTION);
                point
            });
            if points[0].distance_m == points[1].distance_m {
                // 同じ点の場合、後の点を無視する
                writer.write_row(points[0].clone(), block_azimuth as i64);
            } else {
                writer.write_row(points[0].clone(), block_azimuth as i64);
                writer.write_row(points[1].clone(), block_azimuth as i64);
            }
        }
    }
    Ok(())
}


const ROTATION_RESOLUTION: f32 = 0.01;
fn build_velo_point(distance: f32, azimuth: u16, channel: u8, timestamp_ns: u64, intensity: u8, laser_angles: &[f32], distance_resolution: f32) -> VeloPoint {
    let distance_m = distance as f32 * distance_resolution;
    let vertical_angle = laser_angles[channel as usize];
    let omega = vertical_angle.to_radians();
    let alpha = (azimuth as f32 * ROTATION_RESOLUTION).to_radians();

    let x = distance_m * omega.cos() * alpha.sin();
    let y = distance_m * omega.cos() * alpha.cos();
    let z = distance_m * omega.sin();

    VeloPoint {
        intensity,
        channel,
        azimuth,
        distance_m,
        timestamp: timestamp_ns,
        altitude: (vertical_angle * 100.0) as i16,
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
    #[allow(dead_code)]
    num_frames: u16,
    frequency: f32, // Hz
}

fn parse_packet_info(filename: &str) -> Result<PcapInfo, Error> {
    let file = File::open(filename).unwrap();
    let mut reader = LegacyPcapReader::new(65536, file).expect("LegacyPcapReader");

    let mut packet_first_body: Option<Vec<u8>> = None;
    let mut packet_second_body: Option<Vec<u8>> = None;
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
                        if packet_first_body.is_none() {
                            packet_first_body = Some(udp_data.to_vec());
                        }
                        else if packet_second_body.is_none() {
                            packet_second_body = Some(udp_data.to_vec());
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

    let packet_first_body = match packet_first_body {
        Some(body) => body,
        None => return Err(anyhow!("no packet found")),
    };
    let packet_second_body = match packet_second_body {
        Some(body) => body,
        None => return Err(anyhow!("no packet found")),
    };

    ensure!(packet_first_body.len() == 1206, "packet size is not 1206");
    let factory_return_mode = packet_first_body[1204];
    let factory_product_id = packet_first_body[1205];

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
    let first_azimuth = LittleEndian::read_u16(&packet_first_body[2..4]);
    let second_azimuth = LittleEndian::read_u16(&packet_second_body[2..4]);
    let azimuth_diff = if first_azimuth > second_azimuth {
        36000 + second_azimuth - first_azimuth
    } else {
        second_azimuth - first_azimuth
    };
    let elapsed_time_us = LittleEndian::read_u32(&packet_second_body[1200..1204]) - LittleEndian::read_u32(&packet_first_body[1200..1204]);
    let frequency = azimuth_diff as f32 / elapsed_time_us as f32 * 1000.0 / 36.0;

    Ok(PcapInfo {
        return_mode,
        product,
        num_frames,
        frequency,
    })
}
