use crate::{framewriter::FrameWriter, velopoint::VeloPoint};

pub struct TimeSplitWriter {
    pub frame_start_timestamp: u64,
    pub interval: u64,
    pub frame_writer: Box<dyn FrameWriter>,
}

impl TimeSplitWriter {
    pub fn new(interval_ns: u64, frame_writer: Box<dyn FrameWriter>) -> TimeSplitWriter {
        TimeSplitWriter { frame_start_timestamp: 0, interval: interval_ns, frame_writer }
    }

    pub fn write_row(&mut self, row: VeloPoint) {
        let is_new_frame = row.timestamp - self.frame_start_timestamp > self.interval;
        if is_new_frame {
            self.frame_writer.split_frame();
            self.frame_start_timestamp = row.timestamp;
        }
        self.frame_writer.write_row(row);
    }

    pub fn write_attribute(&mut self, laser_num: u32, frequency: f32, return_mode: u32, manufacturer: &str, model: &str) {
        self.frame_writer.write_attribute(laser_num, frequency, return_mode, manufacturer, model);
    }

    pub fn finalize(&mut self) {
        self.frame_writer.split_frame();
    }
}
