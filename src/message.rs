use rkyv::{Archive, Serialize};

#[derive(Archive, Serialize)]
pub struct LogMessage {
    pub timestamp_ns: u64,
    pub metadata_ptr: usize,
    pub logger_ptr: usize,
    pub args_data: Vec<u8>,
}
