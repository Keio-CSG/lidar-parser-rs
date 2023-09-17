use hdf5::H5Type;

#[derive(H5Type, Clone, PartialEq, Debug)] // register with HDF5
#[repr(C)]
pub struct VeloPoint {
    pub reflectivity: u8,
    pub channel: u8,
    pub azimuth: u16,
    pub distance_m: f32,
    pub timestamp: u32,
    pub vertical_angle: f32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl VeloPoint {
    pub fn to_string(&self) -> String {
        format!(
            "{},{},{},{},{},{},{},{},{}\n",
            self.reflectivity,
            self.channel,
            self.azimuth,
            self.distance_m,
            self.timestamp,
            self.vertical_angle,
            self.x,
            self.y,
            self.z,
        )
    }
}