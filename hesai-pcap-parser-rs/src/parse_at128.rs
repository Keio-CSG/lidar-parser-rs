use std::f32::consts::PI;

use writer_common::{azimuthsplitwriter::AzimuthSplitWriter, velopoint::VeloPoint};

use crate::constants_at128::{FIRING_TIMING_OFFSET_OF_EACH_ANGLE, HORIZONTAL_OFFSET, START_FRAME, RESOLUTION, AZIMUTH_ADJUST, ELEVATION_ADJUST, ELEVATION_ANGLE};

pub fn write_header_at128(packet_body: &[u8], writer: &mut AzimuthSplitWriter) {
    let header = &packet_body[6..12];
    let laser_num = header[0] as u32;
    let tail = &packet_body[1046..1086];
    let return_mode = match tail[24] {
        0x37 => 0, // Strongest
        0x38 => 1, // Last
        0x39 => 2, // Dual
        _ => 0,
    };
    let motor_speed = ((tail[19] as u32) << 8) + (tail[18] as u32);
    let frequency = motor_speed as f32 / 60.0;
    writer.write_attribute(laser_num, frequency, return_mode, "Hesai", "AT128");
}

pub fn parse_packet_body_at128(packet_body: &[u8], writer: &mut AzimuthSplitWriter) {
    let header = &packet_body[6..12];
    let block_num = header[1] as u32;

    let body = &packet_body[12..1046];
    let tail = &packet_body[1046..1086];
    let return_mode = tail[24];
    let unix_epoch_sec = ((tail[31] as u64) << 32)
                        + ((tail[30] as u64) << 24) 
                        + ((tail[29] as u64) << 16) 
                        + ((tail[28] as u64) << 8) 
                        + ((tail[27] as u64));
    let timestamp_us = ((tail[23] as u32) << 24) 
                        + ((tail[22] as u32) << 16) 
                        + ((tail[21] as u32) << 8) 
                        + ((tail[20] as u32));
    
    for block_index in 0..block_num {
        let block_timestamp_ns = calc_block_timestamp_ns(unix_epoch_sec, timestamp_us, block_index+1, return_mode);
        let block_start = (block_index*515) as usize;
        parse_block(&body[block_start..block_start+515], block_timestamp_ns, writer);
    }
}

fn calc_block_timestamp_ns(unix_epoch_sec: u64, timestamp_us: u32, block_id: u32, return_mode: u8) -> u64 {
    // unix timeは大きすぎるので、分以下の部分だけを使う
    let t0 = (unix_epoch_sec % 3600) * 1000000000 + timestamp_us as u64 * 1000;
    if return_mode == 0x37 || return_mode == 0x38 {
        t0 + 100000 - 9249 - 41666 * (3 - block_id as u64)
    }
    else {
        t0 + 100000 - 9249 - 41666
    }
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
    let encoder_angle_raw = ((packet_block[1] as u16) << 8) + (packet_block[0] as u16);
    let encoder_fine_angle_raw = packet_block[2];
    for channel in 0..128 as usize {
        let channel_timestamp_ns = block_timestamp_ns + FIRING_TIMING_OFFSET_OF_EACH_ANGLE[channel as usize] as u64;
        let channel_azimuth_deg = calculate_horizontal_angle(encoder_angle_raw, encoder_fine_angle_raw, channel as u8);
        let v_angle = calculate_vertical_angle(channel as u8, encoder_angle_raw, encoder_fine_angle_raw);
        let channel_start = (3+channel*4) as usize;
        let channel_data = &packet_block[channel_start..channel_start+4];
        let distance = ((channel_data[1] as u32) << 8) + (channel_data[0] as u32);
        let reflectivity = channel_data[2];
        let (x,y,z) = calc_polar_coordinate(
            channel_azimuth_deg, 
            v_angle as f32, 
            distance as f32 * 4.0 / 1000.0);

        writer.write_row(VeloPoint { 
            intensity: reflectivity, 
            channel: channel as u8, 
            azimuth: (channel_azimuth_deg * 100.0) as u16, 
            distance_m: distance as f32 * 4.0 / 1000.0,
            timestamp: channel_timestamp_ns, 
            altitude: (v_angle * 100.0) as i16, 
            x, y, z })
    }
}

fn calculate_horizontal_angle(encoder_angle_raw: u16, encoder_fine_angle_raw: u8, channel: u8) -> f32 {
    let encoder_angle = encoder_angle_raw as f32 / 100.0;
    let encoder_fine_angle = encoder_fine_angle_raw as f32 / 100.0 / 256.0;
    let encoder_angle_deg = encoder_angle + encoder_fine_angle;
    let start_frame_angle = get_frame_start_angle_deg(encoder_angle_deg);
    let offset_angle = (HORIZONTAL_OFFSET[channel as usize] * RESOLUTION as i32) as f32 / 25600.0;
    let adjust_angle = get_horizontal_adjust_angle_deg(encoder_angle_deg, channel);

    (encoder_angle_deg - start_frame_angle) * 2.0 - offset_angle + adjust_angle
}

fn get_frame_start_angle_deg(encoder_angle_deg: f32) -> f32 {
    if encoder_angle_deg < (START_FRAME[0] * RESOLUTION) as f32 / 25600.0
    || encoder_angle_deg >= (START_FRAME[2] * RESOLUTION) as f32 / 25600.0 {
        (START_FRAME[2] * RESOLUTION) as f32 / 25600.0
    }
    else if encoder_angle_deg >= (START_FRAME[1] * RESOLUTION) as f32 / 25600.0 {
        (START_FRAME[1] * RESOLUTION) as f32 / 25600.0
    }
    else {
        (START_FRAME[0] * RESOLUTION) as f32 / 25600.0
    }
}

fn get_horizontal_adjust_angle_deg(encoder_angle_deg: f32, channel: u8) -> f32 {
    let lower = (encoder_angle_deg / 2.0).floor() as i32;
    let upper = lower + 1;
    let position = (encoder_angle_deg - (lower * 2) as f32) / 2.0;
    let adjust_angle_lower = AZIMUTH_ADJUST[channel as usize][lower as usize] as f32;
    let adjust_angle_upper = AZIMUTH_ADJUST[channel as usize][upper as usize] as f32;
    (adjust_angle_lower + (adjust_angle_upper - adjust_angle_lower) * position) * RESOLUTION as f32 / 100.0
}

fn calculate_vertical_angle(channel: u8, encoder_angle_raw: u16, encoder_fine_angle_raw: u8) -> f32 {
    let encoder_angle = encoder_angle_raw as f32 / 100.0;
    let encoder_fine_angle = encoder_fine_angle_raw as f32 / 100.0 / 256.0;
    let encoder_angle_deg = encoder_angle + encoder_fine_angle;
    let vertical_angle_deg = ELEVATION_ANGLE[channel as usize] as f32 * RESOLUTION as f32 / 25600.0;
    let adjust_angle = get_vertical_adjust_angle_deg(encoder_angle_deg, channel);
    vertical_angle_deg + adjust_angle
}

fn get_vertical_adjust_angle_deg(encoder_angle_deg: f32, channel: u8) -> f32 {
    let lower = (encoder_angle_deg / 2.0).floor() as i32;
    let upper = lower + 1;
    let position = (encoder_angle_deg - (lower * 2) as f32) / 2.0;
    let adjust_angle_lower = ELEVATION_ADJUST[channel as usize][lower as usize] as f32;
    let adjust_angle_upper = ELEVATION_ADJUST[channel as usize][upper as usize] as f32;
    (adjust_angle_lower + (adjust_angle_upper - adjust_angle_lower) * position) * RESOLUTION as f32 / 100.0
}