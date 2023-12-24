use crate::{framewriter::FrameWriter, velopoint::VeloPoint};

pub struct AzimuthSplitWriter {
    pub previous_azimuth: u16,
    pub min_offset: i32,
    pub writer: Box<dyn FrameWriter>,
}

impl AzimuthSplitWriter {
    pub fn new(writer: Box<dyn FrameWriter>) -> AzimuthSplitWriter {
        AzimuthSplitWriter { previous_azimuth: 0, min_offset: 0, writer }
    }

    pub fn new_with_min_offset(writer: Box<dyn FrameWriter>, min_offset: i32) -> AzimuthSplitWriter {
        AzimuthSplitWriter { previous_azimuth: 0, min_offset, writer }
    }

    pub fn write_row(&mut self, row: VeloPoint, ignore_azimuth: bool) {
        if ignore_azimuth {
            self.writer.write_row(row);
            return;
        }
        let is_new_frame = self.previous_azimuth as i32 - row.azimuth as i32 > self.min_offset;
        if is_new_frame {
            self.writer.split_frame();
        }
        self.previous_azimuth = row.azimuth;
        self.writer.write_row(row);
    }

    pub fn write_attribute(&mut self, laser_num: u32, frequency: f32, return_mode: u32, manufacturer: &str, model: &str) {
        self.writer.write_attribute(laser_num, frequency, return_mode, manufacturer, model);
    }

    pub fn finalize(&mut self) {
        self.writer.split_frame();
    }
}
