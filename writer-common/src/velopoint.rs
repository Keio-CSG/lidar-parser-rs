#![allow(non_upper_case_globals)] // for HDF5 constants

use hdf5::H5Type;


#[derive(H5Type, Clone, PartialEq, Debug)] // register with HDF5
#[repr(C)]
pub struct VeloPoint {
    pub intensity: u8,   // calibrated reflectivity. values: 0-255
    pub channel: u8,     // a.k.a. laser id
    pub timestamp: u64,  // firing time. units: nanoseconds
    pub azimuth: u16,    // horizontal angle. units: 0.01 degrees
    pub altitude: i16,   // vertical angle. units: 0.01 degrees
    pub distance_m: f32, // distance. units: meters
    pub x: f32,          // cartesian coordinates (right-handed coordinate system)
    pub y: f32,          // units: meters
    pub z: f32,          //
}

impl VeloPoint {
    pub fn get_csv_header() -> String {
        "intensity,channel,timestamp,azimuth,altitude,distance_m,x,y,z".to_string()
    }

    pub fn to_csv_string(&self) -> String {
        format!(
            "{},{},{},{},{},{},{},{},{}",
            self.intensity,
            self.channel,
            self.timestamp,
            self.azimuth,
            self.altitude,
            self.distance_m,
            self.x,
            self.y,
            self.z,
        )
    }
}