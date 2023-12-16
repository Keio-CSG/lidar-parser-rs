use std::fs;

use pcd_rs::{DynRecord, ValueKind, DynWriter, WriterInit, Schema, Field};

use crate::{velopoint::VeloPoint, framewriter::FrameWriter};

pub struct PcdWriter {
    base_dir: String,
    dir: String,
    file_prefix: String,
    file_index: u32,
    buffer: Vec<DynRecord>,
}

impl PcdWriter {
    pub fn create(base_dir: String, dir: String, file_prefix: String) -> PcdWriter {
        fs::create_dir(format!("{}/{}", base_dir, dir)).unwrap();
        PcdWriter { 
            base_dir,
            dir, 
            file_prefix, 
            file_index: 0,
            buffer: Vec::new(), 
        }
    }

    fn write_to_file(&mut self) {
        let current_filename = format!("{0}/{1}/{2}_{3:>04}.pcd", self.base_dir, self.dir, self.file_prefix, self.file_index);
        let schema = vec![
            ("x", ValueKind::F32, 1),
            ("y", ValueKind::F32, 1),
            ("z", ValueKind::F32, 1),
            ("intensity", ValueKind::U8, 1),
            ("channel", ValueKind::U8, 1),
            ("azimuth", ValueKind::U16, 1),
            ("altitude", ValueKind::I16, 1),
            ("distance_m", ValueKind::F32, 1),
            ("timestamp", ValueKind::F64, 1),
        ];
        let mut writer: DynWriter<_> = WriterInit {
            width: self.buffer.len() as u64,
            height: 1,
            viewpoint: Default::default(),
            data_kind: pcd_rs::DataKind::Ascii,
            schema: Some(Schema::from_iter(schema)),
        }.create(current_filename).unwrap();
        for point in self.buffer.iter() {
            writer.push(point).unwrap();
        }
        writer.finish().unwrap();
        self.file_index += 1;
    }
}

impl FrameWriter for PcdWriter {
    fn write_row(&mut self, row: VeloPoint) {
        self.buffer.push(DynRecord(vec![
            Field::F32(vec![row.x]),
            Field::F32(vec![row.y]),
            Field::F32(vec![row.z]),
            Field::U8(vec![row.intensity]),
            Field::U8(vec![row.channel]),
            Field::U16(vec![row.azimuth]),
            Field::I16(vec![row.altitude]),
            Field::F32(vec![row.distance_m]),
            Field::F64(vec![row.timestamp as f64]),
        ]));
    }

    fn split_frame(&mut self) { 
        if self.buffer.len() > 0 {
            self.write_to_file();
            self.buffer.clear();
        }
    }

    fn write_attribute(&mut self, _laser_num: u32, _frequency: f32, _return_mode: u32, _manufacturer: &str, _model: &str) { }
}