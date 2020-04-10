use std::path::PathBuf;

pub struct Video {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
    pub size: u64,
}
