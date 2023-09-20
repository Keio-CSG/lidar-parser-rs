use crate::{framewriter::FrameWriter, velopoint::VeloPoint};

pub struct AzimuthSplitWriter {
    pub previous_azimuth: u16,
    pub writer: Box<dyn FrameWriter>,
}

impl AzimuthSplitWriter {
    pub fn new(writer: Box<dyn FrameWriter>) -> AzimuthSplitWriter {
        AzimuthSplitWriter { previous_azimuth: 0, writer }
    }

    pub fn write_row(&mut self, row: VeloPoint) {
        let is_new_frame = row.azimuth < self.previous_azimuth;
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
