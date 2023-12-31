use crate::{
    framewriter::{FrameWriter, ProgressBarExt},
    velopoint::VeloPoint,
};

pub struct ValueSlopeSplitWriter {
    pub has_previous_value: bool,
    pub previous_value: i64,
    pub previous_slope: i64,
    pub min_offset: i32,
    pub writer: Box<dyn FrameWriter>,
    progress_bar: indicatif::ProgressBar,
}

impl ValueSlopeSplitWriter {
    pub fn new(writer: Box<dyn FrameWriter>, frame_num: u64) -> ValueSlopeSplitWriter {
        let progress_bar = indicatif::ProgressBar::new_frame_progress_bar(frame_num);

        ValueSlopeSplitWriter {
            has_previous_value: false,
            previous_value: 0,
            previous_slope: 0,
            min_offset: 0,
            progress_bar,
            writer,
        }
    }

    pub fn new_with_min_offset(
        writer: Box<dyn FrameWriter>,
        frame_num: u64,
        min_offset: i32,
    ) -> ValueSlopeSplitWriter {
        let progress_bar = indicatif::ProgressBar::new_frame_progress_bar(frame_num);
        ValueSlopeSplitWriter {
            has_previous_value: false,
            previous_value: 0,
            previous_slope: 0,
            min_offset,
            progress_bar,
            writer,
        }
    }

    pub fn write_row(&mut self, row: VeloPoint, slope_value: i64) {
        let is_new_frame = self.is_new_frame(slope_value);
        if is_new_frame {
            self.writer.split_frame();
            self.progress_bar.inc(1);
        }
        self.writer.write_row(row);
    }

    fn is_new_frame(&mut self, new_value: i64) -> bool {
        if !self.has_previous_value {
            self.has_previous_value = true;
            self.previous_value = new_value;
            return false;
        }

        let new_slope = new_value - self.previous_value;
        self.previous_value = new_value;
        if new_slope == 0 {
            return false;
        }
        if self.previous_slope == 0 {
            self.previous_slope = new_slope;
            return false;
        }
        let is_slope_same_direction = new_slope.signum() == self.previous_slope.signum();
        if is_slope_same_direction {
            return false;
        } else {
            self.previous_slope = 0;
            return true;
        }
    }

    pub fn write_attribute(
        &mut self,
        laser_num: u32,
        frequency: f32,
        return_mode: u32,
        manufacturer: &str,
        model: &str,
    ) {
        self.writer
            .write_attribute(laser_num, frequency, return_mode, manufacturer, model);
    }

    pub fn finalize(&mut self) {
        self.writer.split_frame();
        self.progress_bar.inc(1);
        self.progress_bar.finish();
    }
}
