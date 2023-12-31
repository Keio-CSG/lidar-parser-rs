use crate::{framewriter::{FrameWriter, ProgressBarExt}, velopoint::VeloPoint};

pub struct TimeSplitWriter {
    pub frame_start_timestamp: u64,
    pub interval: u64,
    pub frame_writer: Box<dyn FrameWriter>,
    progress_bar: indicatif::ProgressBar,
}

impl TimeSplitWriter {
    pub fn new(frame_writer: Box<dyn FrameWriter>, interval_ns: u64, frame_num: u64) -> TimeSplitWriter {
        let progress_bar = indicatif::ProgressBar::new_frame_progress_bar(frame_num);
        TimeSplitWriter { 
            frame_start_timestamp: 0, interval: interval_ns, frame_writer ,
            progress_bar,
        }
    }

    pub fn write_row(&mut self, row: VeloPoint) {
        let is_new_frame = row.timestamp - self.frame_start_timestamp > self.interval;
        if is_new_frame {
            self.frame_writer.split_frame();
            self.frame_start_timestamp = row.timestamp;
            self.progress_bar.inc(1);
        }
        self.frame_writer.write_row(row);
    }

    pub fn write_attribute(&mut self, laser_num: u32, frequency: f32, return_mode: u32, manufacturer: &str, model: &str) {
        self.frame_writer.write_attribute(laser_num, frequency, return_mode, manufacturer, model);
    }

    pub fn finalize(&mut self) {
        self.frame_writer.split_frame();
        self.progress_bar.inc(1);
        self.progress_bar.finish();
    }
}
