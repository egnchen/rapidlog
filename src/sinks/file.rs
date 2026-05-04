use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use crate::sink::Sink;

/// Whether to append to an existing file or truncate it.
pub enum FileMode {
    /// Create the file if missing, append to the end if it exists.
    Append,
    /// Create or overwrite the file.
    Truncate,
}

/// A sink that writes formatted log messages to a file, one per line.
///
/// Uses a buffered writer for performance. Call [`Sink::flush`] to ensure
/// all data is written to disk.
pub struct FileSink {
    file: parking_lot::Mutex<BufWriter<File>>,
    path: PathBuf,
}

impl FileSink {
    pub fn new(path: impl AsRef<Path>, mode: FileMode) -> std::io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file = match mode {
            FileMode::Append => OpenOptions::new().create(true).append(true).open(&path)?,
            FileMode::Truncate => OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&path)?,
        };
        Ok(Self {
            file: parking_lot::Mutex::new(BufWriter::new(file)),
            path,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Sink for FileSink {
    fn write(&self, formatted: &str) {
        let mut file = self.file.lock();
        let _ = writeln!(file, "{formatted}");
    }

    fn flush(&self) {
        let mut file = self.file.lock();
        let _ = file.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("rapidlog_test_file_sink_{name}"))
    }

    fn read_file(path: &Path) -> String {
        let mut f = File::open(path).unwrap();
        let mut contents = String::new();
        f.read_to_string(&mut contents).unwrap();
        contents
    }

    fn cleanup(path: &Path) {
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn write_and_read_back() {
        let p = temp_path("write_read");
        cleanup(&p);
        let sink = FileSink::new(&p, FileMode::Truncate).unwrap();
        sink.write("hello world");
        sink.flush();
        let contents = read_file(&p);
        assert!(contents.contains("hello world"));
        cleanup(&p);
    }

    #[test]
    fn write_multiple_messages() {
        let p = temp_path("multi");
        cleanup(&p);
        let sink = FileSink::new(&p, FileMode::Truncate).unwrap();
        sink.write("first");
        sink.write("second");
        sink.write("third");
        sink.flush();
        let contents = read_file(&p);
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "first");
        assert_eq!(lines[1], "second");
        assert_eq!(lines[2], "third");
        cleanup(&p);
    }

    #[test]
    fn append_mode_preserves() {
        let p = temp_path("append");
        cleanup(&p);
        {
            let sink = FileSink::new(&p, FileMode::Append).unwrap();
            sink.write("line_a");
            sink.flush();
        }
        {
            let sink = FileSink::new(&p, FileMode::Append).unwrap();
            sink.write("line_b");
            sink.flush();
        }
        let contents = read_file(&p);
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "line_a");
        assert_eq!(lines[1], "line_b");
        cleanup(&p);
    }

    #[test]
    fn truncate_mode_overwrites() {
        let p = temp_path("trunc");
        cleanup(&p);
        {
            let sink = FileSink::new(&p, FileMode::Truncate).unwrap();
            sink.write("first_writer");
            sink.flush();
        }
        {
            let sink = FileSink::new(&p, FileMode::Truncate).unwrap();
            sink.write("second_writer");
            sink.flush();
        }
        let contents = read_file(&p);
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "second_writer");
        cleanup(&p);
    }

    #[test]
    fn flush_does_not_panic() {
        let p = temp_path("flush");
        cleanup(&p);
        let sink = FileSink::new(&p, FileMode::Truncate).unwrap();
        sink.flush();
        sink.write("data");
        sink.flush();
        cleanup(&p);
    }

    #[test]
    fn file_sink_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<FileSink>();
        assert_sync::<FileSink>();
    }
}
