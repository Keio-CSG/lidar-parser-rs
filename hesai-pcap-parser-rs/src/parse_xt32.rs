use std::f32::consts::PI;

use writer_common::{azimuthsplitwriter::AzimuthSplitWriter, velopoint::VeloPoint};

pub fn write_header_xt32(packet_body: &[u8], writer: &mut AzimuthSplitWriter) {
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

pub fn parse_packet_body_xt32(packet_body: &[u8], writer: &mut AzimuthSplitWriter) {
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
