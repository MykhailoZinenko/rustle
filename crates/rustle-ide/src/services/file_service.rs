use std::fs;
use std::path::PathBuf;

pub struct FileService;

impl FileService {
    pub fn read(path: &PathBuf) -> String {
        fs::read_to_string(path)
            .unwrap_or_else(|_| String::from("// Failed to read file"))
    }

    pub fn write(path: &PathBuf, content: &str) -> bool {
        fs::write(path, content).is_ok()
    }
}
