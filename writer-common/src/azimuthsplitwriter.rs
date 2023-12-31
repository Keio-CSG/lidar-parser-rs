use crate::{framewriter::{FrameWriter, ProgressBarExt}, velopoint::VeloPoint};

pub struct AzimuthSplitWriter {
    pub previous_azimuth: u16,
    pub min_offset: i32,
    pub writer: Box<dyn FrameWriter>,
    progress_bar: indicatif::ProgressBar,
}

impl AzimuthSplitWriter {
    pub fn new(writer: Box<dyn FrameWriter>, frame_num: u64) -> AzimuthSplitWriter {
        let progress_bar = indicatif::ProgressBar::new_frame_progress_bar(frame_num);
        AzimuthSplitWriter { previous_azimuth: 0, min_offset: 0, writer, progress_bar }
    }

    pub fn new_with_min_offset(writer: Box<dyn FrameWriter>, min_offset: i32, frame_num: u64) -> AzimuthSplitWriter {
        let progress_bar = indicatif::ProgressBar::new_frame_progress_bar(frame_num);
        AzimuthSplitWriter { previous_azimuth: 0, min_offset, writer, progress_bar }
    }

    pub fn write_row(&mut self, row: VeloPoint, ignore_azimuth: bool) {
        if ignore_azimuth {
            self.writer.write_row(row);
            return;
        }
        let is_new_frame = self.previous_azimuth as i32 - row.azimuth as i32 > self.min_offset;
        if is_new_frame {
            self.writer.split_frame();
            self.progress_bar.inc(1);
        }
        self.previous_azimuth = row.azimuth;
        self.writer.write_row(row);
    }

    pub fn write_attribute(&mut self, laser_num: u32, frequency: f32, return_mode: u32, manufacturer: &str, model: &str) {
        self.writer.write_attribute(laser_num, frequency, return_mode, manufacturer, model);
    }

    pub fn finalize(&mut self) {
        self.writer.split_frame();
        self.progress_bar.inc(1);
        self.progress_bar.finish();
    }
}
