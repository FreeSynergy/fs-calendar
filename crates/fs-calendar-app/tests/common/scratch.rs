//! A folder for the files of one test.
//!
//! Pattern: RAII / Guard — removed when the test ends, also when it fails.

use std::path::PathBuf;

use fs_objects::ObjectId;

/// A folder for the files of one test.
pub struct Scratch(PathBuf);

impl Scratch {
    pub fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("fs-calendar-app-{}", ObjectId::new()));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    pub fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
