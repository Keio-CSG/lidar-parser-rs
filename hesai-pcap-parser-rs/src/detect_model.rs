pub enum HesaiModel {
    XT32, AT128, UNKNOWN
}

pub fn detect_model(pre_header: &[u8]) -> HesaiModel {
    let major_version = pre_header[2];
    let minor_version = pre_header[3];
    match (major_version, minor_version) {
        (6, 1) => HesaiModel::XT32,
        (4, 3) => HesaiModel::AT128,
        _ => HesaiModel::UNKNOWN,
    }
}
