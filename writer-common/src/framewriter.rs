use crate::velopoint::VeloPoint;

pub trait FrameWriter {
    fn write_row(&mut self, row: VeloPoint);
    fn finalize(&mut self);
    fn write_attribute(&mut self, laser_num: u32, motor_speed: u32, return_mode: u32, manufacturer: &str, model: &str);
}