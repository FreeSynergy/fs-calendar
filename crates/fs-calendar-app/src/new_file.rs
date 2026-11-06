//! Writing a new file without ever replacing one or leaving half of one.
//!
//! Pattern: Template method — fixed order: a temporary file beside the target, the content, then the
//! target appears at once through a hard link, which refuses a name that is taken (a rename would
//! replace it). Where the file system has no hard links, the target is created directly.

use std::fs::{self, File};
use std::io::{self, ErrorKind, Write};
use std::path::Path;

/// Write `bytes` to `path`, which must not exist yet.
///
/// # Errors
/// A message for the person; nothing is left behind.
pub(crate) fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    write_with(path, |file| file.write_all(bytes))
        .map_err(|e| format!("cannot create {}: {e}", path.display()))
}

fn write_with(path: &Path, content: impl Fn(&mut File) -> io::Result<()>) -> io::Result<()> {
    if path.exists() {
        return Err(io::Error::new(ErrorKind::AlreadyExists, "the file exists"));
    }
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let temporary = path.with_file_name(format!(".{name}.{}.tmp", std::process::id()));
    let done = (|| {
        let mut file = File::create_new(&temporary)?;
        content(&mut file)?;
        file.sync_all()?;
        drop(file);
        match fs::hard_link(&temporary, path) {
            Err(e) if e.kind() != ErrorKind::AlreadyExists => {
                let mut direct = File::create_new(path)?;
                content(&mut direct)
            }
            other => other,
        }
    })();
    let _ = fs::remove_file(&temporary);
    done
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "fs-calendar-new-file-{}",
            fs_store::ObjectId::new()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_failed_write_leaves_nothing_behind() {
        let dir = folder();
        let got = write_with(&dir.join("out.ics"), |file| {
            file.write_all(b"half")?;
            Err(io::Error::other("the disk is full"))
        });
        assert!(got.is_err());
        assert_eq!(
            fs::read_dir(&dir).unwrap().count(),
            0,
            "no target, no temporary file"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_file_appears_whole_and_a_taken_name_is_not_replaced() {
        let dir = folder();
        let path = dir.join("out.ics");
        write_new_file(&path, b"one").unwrap();
        assert!(write_new_file(&path, b"two").is_err());
        assert_eq!(fs::read(&path).unwrap(), b"one");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1, "no temporary file");
        fs::remove_dir_all(dir).unwrap();
    }
}
