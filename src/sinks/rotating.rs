use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::sink::Sink;

/// When to rotate the log file.
pub enum RotationPolicy {
    /// Rotate after writing `max_bytes` to the current file.
    SizeBased { max_bytes: u64 },
    /// Rotate at regular time intervals.
    TimeBased { interval: TimeInterval },
}

/// Interval for time-based log rotation.
pub enum TimeInterval {
    /// Rotate at the top of each hour.
    Hourly,
    /// Rotate at midnight each day.
    Daily,
}

/// A sink that writes to a file and rotates it when a size or time threshold
/// is reached.
///
/// Rotated files are named `{path}.1`, `{path}.2`, etc. The current file
/// always retains its original name. Compression is not yet supported.
pub struct RotatingFileSink {
    inner: parking_lot::Mutex<RotatingState>,
    base_path: PathBuf,
    policy: RotationPolicy,
}

struct RotatingState {
    writer: Option<BufWriter<File>>,
    current_path: PathBuf,
    bytes_written: u64,
    next_rotation_at: SystemTime,
    rotation_index: u64,
}

impl RotatingFileSink {
    pub fn new(base_path: impl AsRef<Path>, policy: RotationPolicy) -> std::io::Result<Self> {
        let base_path = base_path.as_ref().to_path_buf();
        let writer = Self::open_writer(&base_path)?;
        let next_rotation_at = Self::compute_next_rotation(&policy);

        Ok(Self {
            inner: parking_lot::Mutex::new(RotatingState {
                writer: Some(writer),
                current_path: base_path.clone(),
                bytes_written: 0,
                next_rotation_at,
                rotation_index: 0,
            }),
            base_path,
            policy,
        })
    }

    fn open_writer(path: &Path) -> std::io::Result<BufWriter<File>> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(BufWriter::new(file))
    }

    fn compute_next_rotation(policy: &RotationPolicy) -> SystemTime {
        match policy {
            RotationPolicy::SizeBased { .. } => {
                SystemTime::now() + Duration::from_secs(86400 * 365 * 100)
            }
            RotationPolicy::TimeBased { interval } => {
                let now = SystemTime::now();
                let dur = match interval {
                    TimeInterval::Hourly => Duration::from_secs(3600),
                    TimeInterval::Daily => Duration::from_secs(86400),
                };
                now + dur
            }
        }
    }

    fn rotate_inner(
        state: &mut RotatingState,
        policy: &RotationPolicy,
        base_path: &Path,
    ) -> std::io::Result<()> {
        if let Some(mut w) = state.writer.take() {
            // flush buffer; into_inner cannot fail after a successful flush
            let _ = w.flush();
            let _ = w.into_inner().ok();
        }

        state.rotation_index += 1;
        let rotated_name = format!("{}.{}", base_path.display(), state.rotation_index);
        std::fs::rename(&state.current_path, &rotated_name)?;

        state.writer = Some(Self::open_writer(base_path)?);
        state.bytes_written = 0;
        state.next_rotation_at = Self::compute_next_rotation(policy);

        Ok(())
    }
}

impl Sink for RotatingFileSink {
    fn write(&self, formatted: &str) {
        let mut state = self.inner.lock();
        let now = SystemTime::now();
        let add_bytes = formatted.len() as u64 + 1;

        let would_exceed = match &self.policy {
            RotationPolicy::SizeBased { max_bytes } => state.bytes_written + add_bytes > *max_bytes,
            RotationPolicy::TimeBased { .. } => now >= state.next_rotation_at,
        };

        if would_exceed {
            let _ = Self::rotate_inner(&mut state, &self.policy, &self.base_path);
        }

        if let Some(ref mut w) = state.writer {
            let _ = writeln!(w, "{formatted}");
            state.bytes_written += add_bytes;
        }
    }

    fn flush(&self) {
        let mut state = self.inner.lock();
        if let Some(ref mut w) = state.writer {
            let _ = w.flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("rapidlog_test_rotating_{name}"))
    }

    fn cleanup_pattern(base: &Path) {
        let _ = std::fs::remove_file(base);
        for i in 1..20u64 {
            let p = format!("{}.{i}", base.display());
            let _ = std::fs::remove_file(&p);
        }
    }

    fn read_file(path: &Path) -> String {
        let mut f = File::open(path).unwrap();
        let mut contents = String::new();
        f.read_to_string(&mut contents).unwrap();
        contents
    }

    #[test]
    fn create_with_size_policy() {
        let p = temp_path("size_create");
        cleanup_pattern(&p);
        let sink = RotatingFileSink::new(&p, RotationPolicy::SizeBased { max_bytes: 1024 });
        assert!(sink.is_ok());
        cleanup_pattern(&p);
    }

    #[test]
    fn create_with_time_policy() {
        let p = temp_path("time_create");
        cleanup_pattern(&p);
        let sink = RotatingFileSink::new(
            &p,
            RotationPolicy::TimeBased {
                interval: TimeInterval::Hourly,
            },
        );
        assert!(sink.is_ok());
        cleanup_pattern(&p);
    }

    #[test]
    fn size_rotation_triggers() {
        let p = temp_path("size_rot");
        cleanup_pattern(&p);
        let sink = RotatingFileSink::new(&p, RotationPolicy::SizeBased { max_bytes: 20 }).unwrap();

        // Write enough to trigger rotation
        sink.write("this is a long line that will exceed 20 bytes");
        sink.flush();

        // Current file should have been renamed, new file exists
        let rotated = format!("{}.1", p.display());
        assert!(
            Path::new(&rotated).exists(),
            "rotated file not found: {rotated}"
        );
        assert!(p.exists(), "current file not found: {p:?}");

        cleanup_pattern(&p);
    }

    #[test]
    fn time_rotation_triggers() {
        let p = temp_path("time_rot");
        cleanup_pattern(&p);
        let sink = RotatingFileSink::new(
            &p,
            RotationPolicy::TimeBased {
                interval: TimeInterval::Hourly,
            },
        )
        .unwrap();

        // Force rotation by setting next_rotation_at to the past
        {
            let mut state = sink.inner.lock();
            state.next_rotation_at = SystemTime::now() - Duration::from_secs(10);
        }

        sink.write("after rotation");
        sink.flush();

        let rotated = format!("{}.1", p.display());
        assert!(Path::new(&rotated).exists(), "rotated file not found");

        let contents = read_file(&p);
        assert!(contents.contains("after rotation"));

        cleanup_pattern(&p);
    }

    #[test]
    fn rotation_preserves_messages_in_old_file() {
        let p = temp_path("rot_preserve");
        cleanup_pattern(&p);
        let sink = RotatingFileSink::new(&p, RotationPolicy::SizeBased { max_bytes: 100 }).unwrap();

        sink.write("first message");
        sink.flush();

        // Force rotation by setting bytes_written past the limit
        {
            let mut state = sink.inner.lock();
            state.bytes_written = 150;
        }
        sink.write("second message");
        sink.flush();

        let rotated = format!("{}.1", p.display());
        let old_contents = read_file(Path::new(&rotated));
        assert!(old_contents.contains("first message"));

        let new_contents = read_file(&p);
        assert!(new_contents.contains("second message"));

        cleanup_pattern(&p);
    }

    #[test]
    fn rotation_index_increments() {
        let p = temp_path("rot_index");
        cleanup_pattern(&p);
        let sink = RotatingFileSink::new(&p, RotationPolicy::SizeBased { max_bytes: 5 }).unwrap();

        // Trigger three rotations
        for i in 0..3 {
            sink.write("x");
            let mut state = sink.inner.lock();
            state.bytes_written = 10;
            drop(state);
            sink.write(&format!("msg{i}"));
            sink.flush();
        }

        assert!(Path::new(&format!("{}.1", p.display())).exists());
        assert!(Path::new(&format!("{}.2", p.display())).exists());
        assert!(Path::new(&format!("{}.3", p.display())).exists());

        cleanup_pattern(&p);
    }

    #[test]
    fn flush_delegates() {
        let p = temp_path("rot_flush");
        cleanup_pattern(&p);
        let sink =
            RotatingFileSink::new(&p, RotationPolicy::SizeBased { max_bytes: 1024 }).unwrap();

        sink.write("flush me");
        sink.flush();

        let contents = read_file(&p);
        assert!(contents.contains("flush me"));

        cleanup_pattern(&p);
    }

    #[test]
    fn rotating_sink_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<RotatingFileSink>();
        assert_sync::<RotatingFileSink>();
    }
}
