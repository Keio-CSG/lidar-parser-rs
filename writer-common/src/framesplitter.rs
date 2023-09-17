use crate::velopoint::VeloPoint;

pub trait FrameSplitter {
    fn read(&mut self, row: &VeloPoint) -> bool;
}

pub struct AzimuthSplitter {
    pub previous_azimuth: u16,
}

impl AzimuthSplitter {
    pub fn new() -> AzimuthSplitter {
        AzimuthSplitter { previous_azimuth: 0 }
    }
}

impl FrameSplitter for AzimuthSplitter {
    fn read(&mut self, row: &VeloPoint) -> bool {
        let is_new_frame = row.azimuth < self.previous_azimuth;
        self.previous_azimuth = row.azimuth;
        is_new_frame
    }
}

pub struct TimeSplitter {
    pub frame_start_timestamp: u64,
    pub interval: u64,
}

impl TimeSplitter {
    pub fn new(interval_ns: u64) -> TimeSplitter {
        TimeSplitter { frame_start_timestamp: 0, interval: interval_ns }
    }
}

impl FrameSplitter for TimeSplitter {
    fn read(&mut self, row: &VeloPoint) -> bool {
        let is_new_frame = row.timestamp - self.frame_start_timestamp > self.interval;
        if is_new_frame {
            self.frame_start_timestamp = row.timestamp;
        }
        is_new_frame
    }
}