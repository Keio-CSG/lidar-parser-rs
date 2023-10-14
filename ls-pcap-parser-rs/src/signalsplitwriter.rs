use writer_common::{framewriter::FrameWriter, velopoint::VeloPoint};

pub struct SignalSplitWriter {
    pub frame_writer: Box<dyn FrameWriter>,
}

impl SignalSplitWriter {
    pub fn new(frame_writer: Box<dyn FrameWriter>) -> SignalSplitWriter {
        SignalSplitWriter { frame_writer }
    }

    pub fn split_frame(&mut self) {
        self.frame_writer.split_frame();
    }

    pub fn write_row(&mut self, row: VeloPoint) {
        self.frame_writer.write_row(row);
    }

    pub fn write_attribute(&mut self, laser_num: u32, frequency: f32, return_mode: u32, manufacturer: &str, model: &str) {
        self.frame_writer.write_attribute(laser_num, frequency, return_mode, manufacturer, model);
    }

    pub fn finalize(&mut self) {
        self.frame_writer.split_frame();
    }
}
