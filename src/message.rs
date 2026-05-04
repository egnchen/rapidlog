use crate::level::LogLevel;
use crate::logger::Logger;
use crate::metadata::Metadata;

#[derive(rkyv::Archive, rkyv::Serialize, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct LogMessage {
    pub timestamp_ns: u64,
    pub metadata_ptr: u64,
    pub logger_ptr: u64,
    pub args_len: u16,
}

pub const ARCHIVED_HEADER_SIZE: usize = std::mem::size_of::<ArchivedLogMessage>();

const _: () = assert!(ARCHIVED_HEADER_SIZE == 32);

impl LogMessage {
    pub fn new(
        timestamp_ns: u64,
        metadata: &'static Metadata,
        logger: *const Logger,
        args_len: u16,
    ) -> Self {
        Self {
            timestamp_ns,
            metadata_ptr: metadata as *const Metadata as u64,
            logger_ptr: logger as u64,
            args_len,
        }
    }

    pub fn level_from_metadata(&self) -> LogLevel {
        let meta: &Metadata = unsafe { &*(self.metadata_ptr as usize as *const Metadata) };
        meta.level
    }

    pub fn serialize_header_into(&self, buf: &mut [u8]) {
        buf[0..8].copy_from_slice(&self.timestamp_ns.to_le_bytes());
        buf[8..16].copy_from_slice(&self.metadata_ptr.to_le_bytes());
        buf[16..24].copy_from_slice(&self.logger_ptr.to_le_bytes());
        buf[24..26].copy_from_slice(&self.args_len.to_le_bytes());
    }

    pub fn decode(raw: &[u8]) -> Option<ArchivedLogMessage> {
        if raw.len() < ARCHIVED_HEADER_SIZE {
            return None;
        }
        // SAFETY: raw.as_ptr() points to a valid buffer of at least
        // ARCHIVED_HEADER_SIZE bytes (checked above). read_unaligned
        // handles any alignment; the fields are read at their exact
        // rkyv archive offsets.
        unsafe {
            let ptr = raw.as_ptr();
            Some(ArchivedLogMessage {
                timestamp_ns: std::ptr::read_unaligned(ptr as *const u64),
                metadata_ptr: std::ptr::read_unaligned(ptr.add(8) as *const u64),
                logger_ptr: std::ptr::read_unaligned(ptr.add(16) as *const u64),
                args_len: std::ptr::read_unaligned(ptr.add(24) as *const u16),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::Metadata;

    fn test_metadata() -> &'static Metadata {
        Box::leak(Box::new(Metadata::new(
            LogLevel::Info,
            "test {}",
            "test.rs",
            42,
            "test",
        )))
    }

    #[test]
    fn serialize_header_into_works() {
        let meta = test_metadata();
        let msg = LogMessage::new(123456789, meta, std::ptr::null(), 42);
        let mut buf = vec![0u8; ARCHIVED_HEADER_SIZE];
        msg.serialize_header_into(&mut buf);

        let archived = LogMessage::decode(&buf).unwrap();
        assert_eq!(archived.timestamp_ns, 123456789);
        assert_eq!(archived.args_len, 42);
    }

    #[test]
    fn level_from_metadata_works() {
        let meta = test_metadata();
        let msg = LogMessage::new(0, meta, std::ptr::null(), 0);
        assert_eq!(msg.level_from_metadata(), LogLevel::Info);
    }

    #[test]
    fn metadata_pointer_roundtrip() {
        let meta = test_metadata();
        let msg = LogMessage::new(100, meta, std::ptr::null(), 0);
        let mut buf = vec![0u8; ARCHIVED_HEADER_SIZE];
        msg.serialize_header_into(&mut buf);

        let archived = LogMessage::decode(&buf).unwrap();
        let meta_from_archived: &Metadata =
            unsafe { &*(archived.metadata_ptr as usize as *const Metadata) };
        assert_eq!(meta_from_archived.level, LogLevel::Info);
        assert_eq!(meta_from_archived.format_str, "test {}");
    }

    #[test]
    fn decode_invalid_bytes_returns_none() {
        let invalid = vec![0u8; 10];
        assert!(LogMessage::decode(&invalid).is_none());
    }

    #[test]
    fn args_len_preserved() {
        let meta = test_metadata();
        let msg = LogMessage::new(0, meta, std::ptr::null(), 1234);
        let mut buf = vec![0u8; ARCHIVED_HEADER_SIZE];
        msg.serialize_header_into(&mut buf);

        let archived = LogMessage::decode(&buf).unwrap();
        assert_eq!(archived.args_len, 1234);
    }
}
