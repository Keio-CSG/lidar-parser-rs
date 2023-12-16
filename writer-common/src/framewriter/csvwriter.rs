use std::{fs::{OpenOptions, self}, path::Path};
use std::io::{BufWriter, Write};

use crate::{velopoint::VeloPoint, framewriter::FrameWriter};

pub struct CsvWriter {
    base_dir: String,
    dir: String,
    file_prefix: String,
    file_index: u32,
    buffer: Vec<VeloPoint>,
}

impl CsvWriter {
    pub fn create(base_dir: String, dir: String, file_prefix: String) -> CsvWriter {
        fs::create_dir(format!("{}/{}", base_dir, dir)).unwrap();
        CsvWriter { 
            base_dir,
            dir, 
            file_prefix, 
            file_index: 0,
            buffer: Vec::new(), 
        }
    }

    fn write_to_file(&mut self) {
        let current_filename = format!("{0}/{1}/{2}_{3:>04}.csv", self.base_dir, self.dir, self.file_prefix, self.file_index);
        let path = Path::new(&current_filename);
        let mut new_file = BufWriter::with_capacity(262144, OpenOptions::new()
            .create(true)
            .write(true)
            .open(path)
            .unwrap());
        new_file.write(VeloPoint::get_csv_header().as_bytes()).unwrap();
        new_file.write("\n".as_bytes()).unwrap();
        
        new_file.write(self.buffer.iter().map(|x| x.to_csv_string()).collect::<Vec<String>>().join("\n").as_bytes()).unwrap();

        self.file_index += 1;
    }
}

impl FrameWriter for CsvWriter {
    fn write_row(&mut self, row: VeloPoint) {
        self.buffer.push(row);
    }

    fn split_frame(&mut self) { 
        if self.buffer.len() > 0 {
            self.write_to_file();
            self.buffer.clear();
        }
    }

    fn write_attribute(&mut self, _laser_num: u32, _frequency: f32, _return_mode: u32, _manufacturer: &str, _model: &str) { }
}