use std::{
    fs::File,
    io::{BufRead, BufReader, Cursor, Read, Seek},
};

use anyhow::{anyhow, ensure, Error};
use byteorder::{ByteOrder, LittleEndian, ReadBytesExt};
use writer_common::{timesplitwriter::TimeSplitWriter, velopoint::VeloPoint};

pub fn parse_lvx2(
    reader: &mut BufReader<File>,
    frame_time_ms: u64,
    writer: &mut TimeSplitWriter,
) -> Result<(), Error> {
    let mut private_header_block = [0u8; 5];
    reader.read_exact(&mut private_header_block)?;
    let device_count = private_header_block[4];
    ensure!(
        device_count == 1,
        "Unsupported device count: {}",
        device_count
    );
    let mut device_info_block = [0u8; 63];
    reader.read_exact(&mut device_info_block)?;
    // only support device type 9 (Mid-360) and 10 (HAP)
    let device_type = device_info_block[37];
    ensure!(
        device_type == 9 || device_type == 15,
        "Unsupported device type: {}",
        device_type
    );

    // write attrs
    let frequency = 1000.0 / frame_time_ms as f32;
    let model = match device_type {
        9 => "Mid-360",
        15 => "HAP", // 仕様上では10だが、ファイルを見ると15になっている
        _ => unreachable!(),
    };
    writer.write_attribute(0, frequency, 0, "Livox", model);

    loop {
        // read each frame
        if reader.fill_buf()?.is_empty() {
            break;
        }
        let mut frame_header = [0u8; 24];
        reader.read_exact(&mut frame_header)?;
        let current_offset = LittleEndian::read_u64(&frame_header[0..8]);
        let next_offset = LittleEndian::read_u64(&frame_header[8..16]);
        let mut frame_body = vec![0u8; (next_offset - current_offset - 24) as usize];
        reader.read_exact(&mut frame_body)?;

        parse_lvx2_frame_body(&frame_body, writer)?;
    }
    Ok(())
}

fn parse_lvx2_frame_body(buffer: &Vec<u8>, writer: &mut TimeSplitWriter) -> Result<(), Error> {
    let mut cursor = Cursor::new(buffer);
    loop {
        // read each package
        if cursor.position() == buffer.len() as u64 {
            break;
        }
        // skip package header
        // version (1 byte)
        // LiDAR ID (4 bytes)
        // LiDAR_Type (1 byte)
        // timestamp type (1 byte)
        cursor.seek(std::io::SeekFrom::Current(7))?;

        let timestamp = cursor.read_u64::<LittleEndian>()?; // ns

        // Udp Counter (2 bytes)
        cursor.seek(std::io::SeekFrom::Current(2))?;

        let data_type = cursor.read_u8()?;

        let data_length = cursor.read_u32::<LittleEndian>()?;

        // Frame_Counter (1 byte)
        // Reserve (4 bytes)
        cursor.seek(std::io::SeekFrom::Current(5))?;

        match data_type {
            1 => parse_lvx2_data1_list(&mut cursor, timestamp, data_length / 14, writer)?,
            2 => parse_lvx2_data2_list(&mut cursor, timestamp, data_length / 8, writer)?,
            _ => {
                return Err(anyhow!("Unsupported data type: {}", data_type));
            }
        }
    }
    Ok(())
}

/// Parse a package of data type 1
///
/// - x: int32 (mm)
/// - y: int32 (mm)
/// - z: int32 (mm)
/// - reflectivity: uint8
/// - tag: uint8
fn parse_lvx2_data1_list(
    cursor: &mut Cursor<&Vec<u8>>,
    timestamp: u64,
    length: u32,
    writer: &mut TimeSplitWriter,
) -> Result<(), Error> {
    for _ in 0..length {
        let x = cursor.read_i32::<LittleEndian>()? as f32 / 1000.0;
        let y = cursor.read_i32::<LittleEndian>()? as f32 / 1000.0;
        let z = cursor.read_i32::<LittleEndian>()? as f32 / 1000.0;
        let reflectivity = cursor.read_u8()?;
        let tag = cursor.read_u8()?;

        let azimuth = (x.atan2(y) * 18000.0 / std::f32::consts::PI).rem_euclid(36000.0) as u16;
        let altitude = (z.atan2((x * x + y * y).sqrt()) * 18000.0 / std::f32::consts::PI) as i16;
        writer.write_row(VeloPoint {
            intensity: reflectivity,
            channel: tag,
            timestamp,
            azimuth,
            altitude,
            distance_m: ((x * x + y * y + z * z) as f32).sqrt(),
            x,
            y,
            z,
        });
    }
    Ok(())
}

/// Parse a package of data type 2
///
/// - x: int16 (cm)
/// - y: int16 (cm)
/// - z: int16 (cm)
/// - reflectivity: uint8
/// - tag: uint8
fn parse_lvx2_data2_list(
    cursor: &mut Cursor<&Vec<u8>>,
    timestamp: u64,
    length: u32,
    writer: &mut TimeSplitWriter,
) -> Result<(), Error> {
    for _ in 0..length {
        let x = cursor.read_i16::<LittleEndian>()? as f32 / 100.0;
        let y = cursor.read_i16::<LittleEndian>()? as f32 / 100.0;
        let z = cursor.read_i16::<LittleEndian>()? as f32 / 100.0;
        let reflectivity = cursor.read_u8()?;
        let tag = cursor.read_u8()?;

        let azimuth = (x.atan2(y) * 18000.0 / std::f32::consts::PI).rem_euclid(36000.0) as u16;
        let altitude = (z.atan2((x * x + y * y).sqrt()) * 18000.0 / std::f32::consts::PI) as i16;
        writer.write_row(VeloPoint {
            intensity: reflectivity,
            channel: tag,
            timestamp,
            azimuth,
            altitude,
            distance_m: ((x * x + y * y + z * z) as f32).sqrt(),
            x,
            y,
            z,
        });
    }
    Ok(())
}
