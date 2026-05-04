use crate::logger::Logger;
use crate::metadata::Metadata;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ArchivedHeader {
    pub timestamp_ns: u64,
    pub metadata_ptr: u64,
    pub logger_ptr: u64,
    pub _reserved: u64,
}

pub const HEADER_SIZE: usize = std::mem::size_of::<ArchivedHeader>();

const _: () = assert!(HEADER_SIZE == 32);

impl ArchivedHeader {
    pub fn new(timestamp_ns: u64, metadata: &'static Metadata, logger: *const Logger) -> Self {
        Self {
            timestamp_ns,
            metadata_ptr: metadata as *const Metadata as u64,
            logger_ptr: logger as u64,
            _reserved: 0,
        }
    }

    pub fn serialize_into(&self, buf: &mut [u8]) {
        buf[0..8].copy_from_slice(&self.timestamp_ns.to_ne_bytes());
        buf[8..16].copy_from_slice(&self.metadata_ptr.to_ne_bytes());
        buf[16..24].copy_from_slice(&self.logger_ptr.to_ne_bytes());
        buf[24..32].fill(0);
    }

    pub fn decode(raw: &[u8]) -> Option<Self> {
        if raw.len() < HEADER_SIZE {
            return None;
        }
        unsafe {
            let ptr = raw.as_ptr();
            Some(Self {
                timestamp_ns: std::ptr::read_unaligned(ptr as *const u64),
                metadata_ptr: std::ptr::read_unaligned(ptr.add(8) as *const u64),
                logger_ptr: std::ptr::read_unaligned(ptr.add(16) as *const u64),
                _reserved: std::ptr::read_unaligned(ptr.add(24) as *const u64),
            })
        }
    }

    pub fn metadata(&self) -> &Metadata {
        unsafe { &*(self.metadata_ptr as usize as *const Metadata) }
    }

    pub fn logger(&self) -> &Logger {
        unsafe { &*(self.logger_ptr as usize as *const Logger) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::LogLevel;

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
    fn serialize_and_decode() {
        let meta = test_metadata();
        let header = ArchivedHeader::new(123456789, meta, std::ptr::null());
        let mut buf = [0u8; HEADER_SIZE];
        header.serialize_into(&mut buf);

        let decoded = ArchivedHeader::decode(&buf).unwrap();
        assert_eq!(decoded.timestamp_ns, 123456789);
        assert_eq!(decoded.metadata_ptr, header.metadata_ptr);
    }

    #[test]
    fn metadata_roundtrip() {
        let meta = test_metadata();
        let header = ArchivedHeader::new(100, meta, std::ptr::null());
        let mut buf = [0u8; HEADER_SIZE];
        header.serialize_into(&mut buf);

        let decoded = ArchivedHeader::decode(&buf).unwrap();
        assert_eq!(decoded.metadata().level, LogLevel::Info);
        assert_eq!(decoded.metadata().format_str, "test {}");
    }

    #[test]
    fn decode_invalid_bytes_returns_none() {
        assert!(ArchivedHeader::decode(&[0u8; 10]).is_none());
    }

    #[test]
    fn header_size_is_32() {
        assert_eq!(HEADER_SIZE, 32);
    }

    #[test]
    fn reserved_field_is_zero() {
        let meta = test_metadata();
        let header = ArchivedHeader::new(0, meta, std::ptr::null());
        assert_eq!(header._reserved, 0);
    }
}
