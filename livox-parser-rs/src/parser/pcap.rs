use std::io::{Cursor, Seek};

use anyhow::Error;
use byteorder::{LittleEndian, ByteOrder, ReadBytesExt};
use writer_common::{timesplitwriter::TimeSplitWriter, velopoint::VeloPoint};

pub fn write_header(packet_body: &[u8], writer: &mut TimeSplitWriter) {
    // Not implemented
}

pub fn parse_packet_body(packet_body: &[u8], writer: &mut TimeSplitWriter) {
    let header = &packet_body[0..36];
    // let version = header[0]; // 0x00
    // let length = LittleEndian::read_u16(&header[1..3]);
    // let time_interval = LittleEndian::read_u16(&header[3..5]);
    // let dot_num = LittleEndian::read_u16(&header[5..7]);
    // let udp_count = LittleEndian::read_u16(&header[7..9]);
    // let frame_count = header[9];
    let data_type = header[10];
    // let timestamp_type = header[11];
    let timestamp = LittleEndian::read_u64(&header[28..36]);

    let mut cursor = Cursor::new(&packet_body[36..]);

    match data_type {
        0x00 => {
            parse_data0(&mut cursor, writer).unwrap();
        },
        0x01 => {
            parse_data1_list(&mut cursor, timestamp, writer).unwrap();
        },
        0x02 => {
            parse_data2_list(&mut cursor, timestamp, writer).unwrap();
        },
        0x03 => {
            parse_data3_list(&mut cursor, timestamp, writer).unwrap();
        },
        _ => {
            eprintln!("Unsupported data type: {}", data_type);
            std::process::exit(1);
        }
    }
}

/// Parse a data type 0
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
fn parse_data0(cursor: &mut Cursor<&[u8]>, writer: &mut TimeSplitWriter) -> Result<(), Error> {
    cursor.seek(std::io::SeekFrom::Current(24))?; // skip 6 * 4 bytes
    Ok(())
}

/// Parse a data type 1
/// 
/// - x: int32 (mm)
/// - y: int32 (mm)
/// - z: int32 (mm)
/// - reflectivity: uint8
/// - tag: uint8
fn parse_data1_list(cursor: &mut Cursor<&[u8]>, timestamp: u64, writer: &mut TimeSplitWriter) -> Result<(), Error> {
    for _ in 0..96 {
        let x = cursor.read_i32::<LittleEndian>()? as f32 / 1000.0;
        let y = cursor.read_i32::<LittleEndian>()? as f32 / 1000.0;
        let z = cursor.read_i32::<LittleEndian>()? as f32 / 1000.0;
        let reflectivity = cursor.read_u8()?;
        let tag = cursor.read_u8()?;

        let azimuth = (x.atan2(y) * 18000.0 / std::f32::consts::PI) % 36000.0;
        let azimuth = if azimuth < 0.0 { (azimuth + 36000.0) as u16 } else { azimuth as u16 };
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

/// Parse a data type 2
/// 
/// - x: int16 (cm)
/// - y: int16 (cm)
/// - z: int16 (cm)
/// - reflectivity: uint8
/// - tag: uint8
fn parse_data2_list(cursor: &mut Cursor<&[u8]>, timestamp: u64, writer: &mut TimeSplitWriter) -> Result<(), Error> {
    for _ in 0..96 {
        let x = cursor.read_i16::<LittleEndian>()? as f32 / 100.0;
        let y = cursor.read_i16::<LittleEndian>()? as f32 / 100.0;
        let z = cursor.read_i16::<LittleEndian>()? as f32 / 100.0;
        let reflectivity = cursor.read_u8()?;
        let tag = cursor.read_u8()?;

        let azimuth = (x.atan2(y) * 18000.0 / std::f32::consts::PI) % 36000.0;
        let azimuth = if azimuth < 0.0 { (azimuth + 36000.0) as u16 } else { azimuth as u16 };
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

/// Parse a data type 3
/// 
/// - depth: uint32 (mm)
/// - theta: uint16 (0.01 degree) [0, 18000]
/// - phi: uint16 (0.01 degree) [0, 36000]
/// - reflectivity: uint8
/// - tag: uint8
fn parse_data3_list(cursor: &mut Cursor<&[u8]>, timestamp: u64, writer: &mut TimeSplitWriter) -> Result<(), Error> {
    for _ in 0..96 {
        let depth = cursor.read_u32::<LittleEndian>()? as f32 / 1000.0;
        let theta = cursor.read_u16::<LittleEndian>()? as f32 / 100.0;
        let phi = cursor.read_u16::<LittleEndian>()? as f32 / 100.0;
        let reflectivity = cursor.read_u8()?;
        let tag = cursor.read_u8()?;

        let azimuth = (phi % 36000.0) as u16;
        let altitude = theta as i16;
        let x = depth * theta.to_radians().cos() * phi.to_radians().sin();
        let y = depth * theta.to_radians().cos() * phi.to_radians().cos();
        let z = depth * theta.to_radians().sin();
        writer.write_row(VeloPoint {
            intensity: reflectivity,
            channel: tag,
            timestamp,
            azimuth,
            altitude,
            distance_m: depth,
            x,
            y,
            z,
        });
    }
    Ok(())
}