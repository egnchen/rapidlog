use rkyv::{Archive, CheckBytes, Serialize};

use crate::level::LogLevel;
use crate::logger::Logger;
use crate::metadata::Metadata;

#[derive(Archive, Serialize, CheckBytes, Debug, PartialEq, Eq)]
#[archive_attr(derive(CheckBytes))]
pub struct LogMessage {
    pub timestamp_ns: u64,
    pub metadata_ptr: u64,
    pub logger_ptr: u64,
    pub args_data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum EncodeError {
    SerializationFailed,
}

const STACK_BUFFER_SIZE: usize = 256;

impl LogMessage {
    pub fn new(
        timestamp_ns: u64,
        metadata: &'static Metadata,
        logger: *const Logger,
        args_data: Vec<u8>,
    ) -> Self {
        Self {
            timestamp_ns,
            metadata_ptr: metadata as *const Metadata as u64,
            logger_ptr: logger as u64,
            args_data,
        }
    }

    pub fn level_from_metadata(&self) -> LogLevel {
        let meta: &Metadata = unsafe { &*(self.metadata_ptr as usize as *const Metadata) };
        meta.level
    }

    pub fn encode(&self) -> Result<Vec<u8>, EncodeError> {
        rkyv::to_bytes::<_, STACK_BUFFER_SIZE>(self)
            .map(|v| v.to_vec())
            .map_err(|_| EncodeError::SerializationFailed)
    }

    pub fn decode(bytes: &[u8]) -> Option<&ArchivedLogMessage> {
        rkyv::check_archived_root::<LogMessage>(bytes).ok()
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
    fn encode_decode_roundtrip() {
        let meta = test_metadata();
        let msg = LogMessage::new(123456789, meta, std::ptr::null(), vec![1, 2, 3, 4]);

        let bytes = msg.encode().unwrap();
        let archived = LogMessage::decode(&bytes).unwrap();

        assert_eq!(archived.timestamp_ns, 123456789);
        assert_eq!(archived.args_data, vec![1u8, 2, 3, 4]);
    }

    #[test]
    fn encode_empty_message() {
        let meta = test_metadata();
        let msg = LogMessage::new(0, meta, std::ptr::null(), vec![]);

        let bytes = msg.encode().unwrap();
        let archived = LogMessage::decode(&bytes).unwrap();

        assert_eq!(archived.timestamp_ns, 0);
        assert!(archived.args_data.is_empty());
    }

    #[test]
    fn level_from_metadata_works() {
        let meta = test_metadata();
        let msg = LogMessage::new(0, meta, std::ptr::null(), vec![]);
        assert_eq!(msg.level_from_metadata(), LogLevel::Info);
    }

    #[test]
    fn args_data_roundtrip() {
        let meta = test_metadata();
        let data = vec![255u8; 1000];
        let msg = LogMessage::new(42, meta, std::ptr::null(), data.clone());

        let bytes = msg.encode().unwrap();
        let archived = LogMessage::decode(&bytes).unwrap();

        assert_eq!(archived.args_data, data);
    }

    #[test]
    fn metadata_pointer_roundtrip() {
        let meta = test_metadata();
        let msg = LogMessage::new(0, meta, std::ptr::null(), vec![]);

        let bytes = msg.encode().unwrap();
        let archived = LogMessage::decode(&bytes).unwrap();

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
}
