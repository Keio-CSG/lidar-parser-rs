use crate::velopoint::VeloPoint;

mod csvwriter;
mod hdfwriter;

pub use csvwriter::*;
pub use hdfwriter::*;

pub trait FrameWriter {
    fn write_row(&mut self, row: VeloPoint);
    fn split_frame(&mut self);
    fn write_attribute(&mut self, laser_num: u32, frequency: f32, return_mode: u32, manufacturer: &str, model: &str);
}
