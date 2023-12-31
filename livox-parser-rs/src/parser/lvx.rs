use std::{
    fs::File,
    io::{BufRead, BufReader, Cursor, Read, Seek},
};

use anyhow::{anyhow, ensure, Error};
use byteorder::{ByteOrder, LittleEndian, ReadBytesExt};
use writer_common::{timesplitwriter::TimeSplitWriter, velopoint::VeloPoint};

pub fn parse_lvx(
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
    let mut device_info_block = [0u8; 59];
    reader.read_exact(&mut device_info_block)?;
    // only support device type 3 (Horizon)
    let device_type = device_info_block[33];
    ensure!(device_type == 3, "Unsupported device type: {}", device_type);

    // write attrs
    let frequency = 1000.0 / frame_time_ms as f32;
    let model = "Horizon";
    let buf = reader.fill_buf()?;
    let data_type = buf[42];
    let return_mode = match data_type {
        0 | 1 | 2 | 3 => 0,
        4 | 5 => 2,
        _ => {
            return Err(anyhow!("Unsupported first data type: {}", data_type));
        }
    };

    writer.write_attribute(0, frequency, return_mode, "Livox", model);

    loop {
        // read each frame
        if reader.fill_buf()?.is_empty() {
            break;
        }
        // frame headerのサイズは仕様書では32バイトだが、実際には24バイトしかない
        let mut frame_header = [0u8; 24];
        reader.read_exact(&mut frame_header)?;
        let current_offset = LittleEndian::read_u64(&frame_header[0..8]);
        let next_offset = LittleEndian::read_u64(&frame_header[8..16]);
        // println!("current_offset: {}, next_offset: {}", current_offset, next_offset);
        let mut frame_body = vec![0u8; (next_offset - current_offset - 24) as usize];
        reader.read_exact(&mut frame_body)?;

        parse_lvx_frame_body(&frame_body, writer)?;
    }
    Ok(())
}

fn parse_lvx_frame_body(buffer: &Vec<u8>, writer: &mut TimeSplitWriter) -> Result<(), Error> {
    let mut cursor = Cursor::new(buffer);
    loop {
        // read each package
        if cursor.position() == buffer.len() as u64 {
            break;
        }
        // skip package header
        // device Index (1 byte)
        // version (1 byte)
        // slot ID (1 byte)
        // LiDAR ID (1 byte)
        // rsvd (1 byte)
        // status code (4 bytes)
        // timestamp type (1 byte)
        cursor.seek(std::io::SeekFrom::Current(10))?;

        let data_type = cursor.read_u8()?;
        let timestamp = cursor.read_u64::<LittleEndian>()?; // ns

        match data_type {
            0 => parse_lvx_data0_list(&mut cursor, timestamp, writer)?,
            1 => parse_lvx_data1_list(&mut cursor, timestamp, writer)?,
            2 => parse_lvx_data2_list(&mut cursor, timestamp, writer)?,
            3 => parse_lvx_data3_list(&mut cursor, timestamp, writer)?,
            4 => parse_lvx_data4_list(&mut cursor, timestamp, writer)?,
            5 => parse_lvx_data5_list(&mut cursor, timestamp, writer)?,
            6 => parse_lvx_data6_list(&mut cursor, timestamp, writer)?,
            _ => {
                return Err(anyhow!(
                    "Unsupported data type: {} at {}",
                    data_type,
                    cursor.position()
                ));
            }
        }
    }
    Ok(())
}

/// Parse a package of data type 0
///
/// format: Cartesian Coordinate System; Single Return; (Only for MID)
///
/// 100 points per package
///
/// - x: int32 (mm)
/// - y: int32 (mm)
/// - z: int32 (mm)
/// - reflectivity: uint8
fn parse_lvx_data0_list(
    cursor: &mut Cursor<&Vec<u8>>,
    timestamp: u64,
    writer: &mut TimeSplitWriter,
) -> Result<(), Error> {
    for _ in 0..100 {
        let x = cursor.read_i32::<LittleEndian>()? as f32 / 1000.0;
        let y = cursor.read_i32::<LittleEndian>()? as f32 / 1000.0;
        let z = cursor.read_i32::<LittleEndian>()? as f32 / 1000.0;
        let reflectivity = cursor.read_u8()?;
        let azimuth = (x.atan2(y) * 18000.0 / std::f32::consts::PI).rem_euclid(36000.0) as u16;
        let altitude = (z.atan2((x * x + y * y).sqrt()) * 18000.0 / std::f32::consts::PI) as i16;
        writer.write_row(VeloPoint {
            intensity: reflectivity,
            channel: 0,
            timestamp,
            azimuth,
            altitude,
            distance_m: (x * x + y * y + z * z).sqrt(),
            x,
            y,
            z,
        });
    }
    Ok(())
}

/// Parse a package of data type 1
///
/// format: Spherical Coordinate System; Single Return; (Only for MID)
///
/// 100 points per package
///
/// - depth: int32 (mm)
/// - theta: uint16 (0.01 degree)
/// - phi: uint16 (0.01 degree)
/// - reflectivity: uint8
fn parse_lvx_data1_list(
    cursor: &mut Cursor<&Vec<u8>>,
    timestamp: u64,
    writer: &mut TimeSplitWriter,
) -> Result<(), Error> {
    for _ in 0..100 {
        let depth = cursor.read_i32::<LittleEndian>()?;
        let theta = cursor.read_u16::<LittleEndian>()?;
        let phi = cursor.read_u16::<LittleEndian>()?;
        let reflectivity = cursor.read_u8()?;
        let azimuth = (-(phi as i32) + 9000).rem_euclid(36000) as u16;
        let altitude = -(theta as i16) + 9000;
        let distance_m = depth as f32 / 1000.0;

        let x = distance_m
            * (altitude as f32 * std::f32::consts::PI / 18000.0).cos()
            * (azimuth as f32 * std::f32::consts::PI / 18000.0).sin();
        let y = distance_m
            * (altitude as f32 * std::f32::consts::PI / 18000.0).cos()
            * (azimuth as f32 * std::f32::consts::PI / 18000.0).cos();
        let z = distance_m * (altitude as f32 * std::f32::consts::PI / 18000.0).sin();

        writer.write_row(VeloPoint {
            intensity: reflectivity,
            channel: 0,
            timestamp,
            azimuth,
            altitude,
            distance_m,
            x,
            y,
            z,
        });
    }
    Ok(())
}

/// Parse a package of data type 2
///
/// format: Cartesian Coordinate System; Single Return;
///
/// 96 points per package
///
/// - x: int32 (mm)
/// - y: int32 (mm)
/// - z: int32 (mm)
/// - reflectivity: uint8
/// - tag: uint8
fn parse_lvx_data2_list(
    cursor: &mut Cursor<&Vec<u8>>,
    timestamp: u64,
    writer: &mut TimeSplitWriter,
) -> Result<(), Error> {
    for _ in 0..96 {
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
            distance_m: (x * x + y * y + z * z).sqrt(),
            x,
            y,
            z,
        });
    }
    Ok(())
}

/// Parse a package of data type 3
///
/// format: Spherical Coordinate System; Single Return;
///
/// 96 points per package
///
/// - depth: int32 (mm)
/// - theta: uint16 (0.01 degree)
/// - phi: uint16 (0.01 degree)
/// - reflectivity: uint8
/// - tag: uint8
fn parse_lvx_data3_list(
    cursor: &mut Cursor<&Vec<u8>>,
    timestamp: u64,
    writer: &mut TimeSplitWriter,
) -> Result<(), Error> {
    for _ in 0..96 {
        let depth = cursor.read_i32::<LittleEndian>()?;
        let theta = cursor.read_u16::<LittleEndian>()?;
        let phi = cursor.read_u16::<LittleEndian>()?;
        let reflectivity = cursor.read_u8()?;
        let tag = cursor.read_u8()?;
        let azimuth = (-(phi as i32) + 9000).rem_euclid(36000) as u16;
        let altitude = -(theta as i16) + 9000;
        let distance_m = depth as f32 / 1000.0;

        let x = distance_m
            * (altitude as f32 * std::f32::consts::PI / 18000.0).cos()
            * (azimuth as f32 * std::f32::consts::PI / 18000.0).sin();
        let y = distance_m
            * (altitude as f32 * std::f32::consts::PI / 18000.0).cos()
            * (azimuth as f32 * std::f32::consts::PI / 18000.0).cos();
        let z = distance_m * (altitude as f32 * std::f32::consts::PI / 18000.0).sin();

        writer.write_row(VeloPoint {
            intensity: reflectivity,
            channel: tag,
            timestamp,
            azimuth,
            altitude,
            distance_m,
            x,
            y,
            z,
        });
    }
    Ok(())
}

/// Parse a package of data type 4
///
/// format: Cartesian Coordinate System; Double Return;
///
/// 48 points per package
///
/// - x1: int32 (mm)
/// - y1: int32 (mm)
/// - z1: int32 (mm)
/// - reflectivity1: uint8
/// - tag1: uint8
/// - x2: int32 (mm)
/// - y2: int32 (mm)
/// - z2: int32 (mm)
/// - reflectivity2: uint8
/// - tag2: uint8
fn parse_lvx_data4_list(
    cursor: &mut Cursor<&Vec<u8>>,
    timestamp: u64,
    writer: &mut TimeSplitWriter,
) -> Result<(), Error> {
    for _ in 0..48 {
        for _ in 0..2 {
            let x = cursor.read_i32::<LittleEndian>()? as f32 / 1000.0;
            let y = cursor.read_i32::<LittleEndian>()? as f32 / 1000.0;
            let z = cursor.read_i32::<LittleEndian>()? as f32 / 1000.0;
            let reflectivity = cursor.read_u8()?;
            let tag = cursor.read_u8()?;

            let azimuth = (x.atan2(y) * 18000.0 / std::f32::consts::PI).rem_euclid(36000.0) as u16;
            let altitude =
                (z.atan2((x * x + y * y).sqrt()) * 18000.0 / std::f32::consts::PI) as i16;
            writer.write_row(VeloPoint {
                intensity: reflectivity,
                channel: tag,
                timestamp,
                azimuth,
                altitude,
                distance_m: (x * x + y * y + z * z).sqrt(),
                x,
                y,
                z,
            });
        }
    }
    Ok(())
}

/// Parse a package of data type 5
///
/// format: Spherical Coordinate System; Double Return;
///
/// 48 points per package
///
/// - theta: uint16 (0.01 degree)
/// - phi: uint16 (0.01 degree)
/// - depth1: uint32 (mm)
/// - reflectivity1: uint8
/// - tag1: uint8
/// - depth2: uint32 (mm)
/// - reflectivity2: uint8
/// - tag2: uint8
fn parse_lvx_data5_list(
    cursor: &mut Cursor<&Vec<u8>>,
    timestamp: u64,
    writer: &mut TimeSplitWriter,
) -> Result<(), Error> {
    for _ in 0..48 {
        let theta = cursor.read_u16::<LittleEndian>()?;
        let phi = cursor.read_u16::<LittleEndian>()?;
        for _ in 0..2 {
            let depth = cursor.read_u32::<LittleEndian>()?;
            let reflectivity = cursor.read_u8()?;
            let tag = cursor.read_u8()?;
            let azimuth = (-(phi as i32) + 9000).rem_euclid(36000) as u16;
            let altitude = -(theta as i16) + 9000;
            let distance_m = depth as f32 / 1000.0;

            let x = distance_m
                * (altitude as f32 * std::f32::consts::PI / 18000.0).cos()
                * (azimuth as f32 * std::f32::consts::PI / 18000.0).sin();
            let y = distance_m
                * (altitude as f32 * std::f32::consts::PI / 18000.0).cos()
                * (azimuth as f32 * std::f32::consts::PI / 18000.0).cos();
            let z = distance_m * (altitude as f32 * std::f32::consts::PI / 18000.0).sin();

            writer.write_row(VeloPoint {
                intensity: reflectivity,
                channel: tag,
                timestamp,
                azimuth,
                altitude,
                distance_m,
                x,
                y,
                z,
            });
        }
    }
    Ok(())
}

/// Parse a package of data type 6
///
/// format: IMU Information
///
/// 1 point per package
///
/// - gyro_x: float32 (rad/s)
/// - gyro_y: float32 (rad/s)
/// - gyro_z: float32 (rad/s)
/// - acc_x: float32 (g)
/// - acc_y: float32 (g)
/// - acc_z: float32 (g)
fn parse_lvx_data6_list(
    cursor: &mut Cursor<&Vec<u8>>,
    _timestamp: u64,
    _writer: &mut TimeSplitWriter,
) -> Result<(), Error> {
    cursor.seek(std::io::SeekFrom::Current(24))?; // skip 6 * 4 bytes
    Ok(())
}
