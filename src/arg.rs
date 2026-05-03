pub const TAG_INT: u8 = 0;
pub const TAG_FLOAT: u8 = 1;
pub const TAG_STR: u8 = 2;
pub const TAG_FORMATTED: u8 = 3;

pub trait LogArg {
    fn log_tag(&self) -> u8;
    fn log_encode(&self, buf: &mut [u8]) -> usize;
    fn log_max_size(&self) -> usize;
}

#[derive(Debug, Clone)]
pub enum DecodedArg {
    Int(i64),
    Float(f64),
    Str(String),
    Formatted(String),
}

impl DecodedArg {
    pub fn as_display_string(&self) -> String {
        match self {
            Self::Int(v) => v.to_string(),
            Self::Float(v) => v.to_string(),
            Self::Str(v) => v.clone(),
            Self::Formatted(v) => v.clone(),
        }
    }
}

impl LogArg for i32 {
    fn log_tag(&self) -> u8 {
        TAG_INT
    }
    fn log_encode(&self, buf: &mut [u8]) -> usize {
        buf[..8].copy_from_slice(&(*self as i64).to_le_bytes());
        8
    }
    fn log_max_size(&self) -> usize {
        8
    }
}

impl LogArg for i64 {
    fn log_tag(&self) -> u8 {
        TAG_INT
    }
    fn log_encode(&self, buf: &mut [u8]) -> usize {
        buf[..8].copy_from_slice(&self.to_le_bytes());
        8
    }
    fn log_max_size(&self) -> usize {
        8
    }
}

impl LogArg for u32 {
    fn log_tag(&self) -> u8 {
        TAG_INT
    }
    fn log_encode(&self, buf: &mut [u8]) -> usize {
        buf[..8].copy_from_slice(&(*self as i64).to_le_bytes());
        8
    }
    fn log_max_size(&self) -> usize {
        8
    }
}

impl LogArg for u64 {
    fn log_tag(&self) -> u8 {
        TAG_INT
    }
    fn log_encode(&self, buf: &mut [u8]) -> usize {
        buf[..8].copy_from_slice(&(*self as i64).to_le_bytes());
        8
    }
    fn log_max_size(&self) -> usize {
        8
    }
}

impl LogArg for usize {
    fn log_tag(&self) -> u8 {
        TAG_INT
    }
    fn log_encode(&self, buf: &mut [u8]) -> usize {
        buf[..8].copy_from_slice(&(*self as i64).to_le_bytes());
        8
    }
    fn log_max_size(&self) -> usize {
        8
    }
}

impl LogArg for f32 {
    fn log_tag(&self) -> u8 {
        TAG_FLOAT
    }
    fn log_encode(&self, buf: &mut [u8]) -> usize {
        buf[..8].copy_from_slice(&(*self as f64).to_le_bytes());
        8
    }
    fn log_max_size(&self) -> usize {
        8
    }
}

impl LogArg for f64 {
    fn log_tag(&self) -> u8 {
        TAG_FLOAT
    }
    fn log_encode(&self, buf: &mut [u8]) -> usize {
        buf[..8].copy_from_slice(&self.to_le_bytes());
        8
    }
    fn log_max_size(&self) -> usize {
        8
    }
}

impl LogArg for bool {
    fn log_tag(&self) -> u8 {
        TAG_INT
    }
    fn log_encode(&self, buf: &mut [u8]) -> usize {
        buf[..8].copy_from_slice(&(if *self { 1i64 } else { 0i64 }).to_le_bytes());
        8
    }
    fn log_max_size(&self) -> usize {
        8
    }
}

impl LogArg for &str {
    fn log_tag(&self) -> u8 {
        TAG_STR
    }
    fn log_encode(&self, buf: &mut [u8]) -> usize {
        let bytes = self.as_bytes();
        let len = bytes.len().min(u16::MAX as usize) as u16;
        buf[..2].copy_from_slice(&len.to_le_bytes());
        buf[2..2 + len as usize].copy_from_slice(&bytes[..len as usize]);
        2 + len as usize
    }
    fn log_max_size(&self) -> usize {
        2 + self.len()
    }
}

impl LogArg for String {
    fn log_tag(&self) -> u8 {
        TAG_STR
    }
    fn log_encode(&self, buf: &mut [u8]) -> usize {
        self.as_str().log_encode(buf)
    }
    fn log_max_size(&self) -> usize {
        2 + self.len()
    }
}

pub fn encode_formatted(buf: &mut [u8], formatted: &str) -> usize {
    let bytes = formatted.as_bytes();
    let len = bytes.len().min(u32::MAX as usize) as u32;
    buf[..4].copy_from_slice(&len.to_le_bytes());
    buf[4..4 + len as usize].copy_from_slice(&bytes[..len as usize]);
    4 + len as usize
}

pub struct DisplayArg<T: std::fmt::Display>(pub T);

impl<T: std::fmt::Display> LogArg for DisplayArg<T> {
    fn log_tag(&self) -> u8 {
        TAG_FORMATTED
    }
    fn log_encode(&self, buf: &mut [u8]) -> usize {
        encode_formatted(buf, &self.0.to_string())
    }
    fn log_max_size(&self) -> usize {
        512
    }
}

pub struct DebugArg<T: std::fmt::Debug>(pub T);

impl<T: std::fmt::Debug> LogArg for DebugArg<T> {
    fn log_tag(&self) -> u8 {
        TAG_FORMATTED
    }
    fn log_encode(&self, buf: &mut [u8]) -> usize {
        encode_formatted(buf, &format!("{:?}", self.0))
    }
    fn log_max_size(&self) -> usize {
        512
    }
}

pub fn decode_args(payload: &[u8]) -> Vec<DecodedArg> {
    if payload.is_empty() {
        return vec![];
    }
    let arg_count = payload[0] as usize;
    if arg_count == 0 || 1 + arg_count > payload.len() {
        return vec![];
    }
    let tags = &payload[1..1 + arg_count];
    let data = &payload[1 + arg_count..];
    let mut data_pos = 0usize;
    let mut args = Vec::with_capacity(arg_count);

    for &tag in tags {
        if data_pos >= data.len() {
            break;
        }
        match tag {
            TAG_INT => {
                if data_pos + 8 > data.len() {
                    break;
                }
                let val = i64::from_le_bytes(data[data_pos..data_pos + 8].try_into().unwrap());
                data_pos += 8;
                args.push(DecodedArg::Int(val));
            }
            TAG_FLOAT => {
                if data_pos + 8 > data.len() {
                    break;
                }
                let val = f64::from_le_bytes(data[data_pos..data_pos + 8].try_into().unwrap());
                data_pos += 8;
                args.push(DecodedArg::Float(val));
            }
            TAG_STR => {
                if data_pos + 2 > data.len() {
                    break;
                }
                let len =
                    u16::from_le_bytes(data[data_pos..data_pos + 2].try_into().unwrap()) as usize;
                data_pos += 2;
                if data_pos + len > data.len() {
                    break;
                }
                let s = std::str::from_utf8(&data[data_pos..data_pos + len])
                    .unwrap_or("<invalid utf8>")
                    .to_owned();
                data_pos += len;
                args.push(DecodedArg::Str(s));
            }
            TAG_FORMATTED => {
                if data_pos + 4 > data.len() {
                    break;
                }
                let len =
                    u32::from_le_bytes(data[data_pos..data_pos + 4].try_into().unwrap()) as usize;
                data_pos += 4;
                if data_pos + len > data.len() {
                    break;
                }
                let s = std::str::from_utf8(&data[data_pos..data_pos + len])
                    .unwrap_or("<invalid utf8>")
                    .to_owned();
                data_pos += len;
                args.push(DecodedArg::Formatted(s));
            }
            _ => break,
        }
    }
    args
}

pub fn format_with_args(format_str: &str, args: &[DecodedArg]) -> String {
    let mut result = String::new();
    let mut remaining = format_str;
    let mut arg_idx = 0;

    while let Some(brace) = remaining.find('{') {
        result.push_str(&remaining[..brace]);
        let after_brace = &remaining[brace + 1..];
        let Some(close) = after_brace.find('}') else {
            result.push_str(remaining);
            remaining = "";
            break;
        };
        let _spec = &after_brace[..close];

        if arg_idx < args.len() {
            result.push_str(&args[arg_idx].as_display_string());
            arg_idx += 1;
        } else {
            result.push_str(&remaining[brace..brace + close + 2]);
        }
        remaining = &after_brace[close + 1..];
    }
    result.push_str(remaining);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_int() {
        let val: i32 = 42;
        let tag = val.log_tag();
        assert_eq!(tag, TAG_INT);
        let mut buf = [0u8; 16];
        let wrote = val.log_encode(&mut buf);
        assert_eq!(wrote, 8);
    }

    #[test]
    fn roundtrip_float() {
        let val: f64 = 3.14;
        let tag = val.log_tag();
        assert_eq!(tag, TAG_FLOAT);
        let mut buf = [0u8; 16];
        let wrote = val.log_encode(&mut buf);
        assert_eq!(wrote, 8);
    }

    #[test]
    fn roundtrip_str() {
        let val: &str = "hello";
        let tag = val.log_tag();
        assert_eq!(tag, TAG_STR);
        let mut buf = [0u8; 32];
        let wrote = val.log_encode(&mut buf);
        assert_eq!(wrote, 2 + 5);
        let len = u16::from_le_bytes(buf[..2].try_into().unwrap());
        assert_eq!(len, 5);
        assert_eq!(&buf[2..7], b"hello");
    }

    #[test]
    fn decode_packed_args() {
        let mut buf = [0u8; 64];
        buf[0] = 3; // arg_count
        buf[1] = TAG_INT;
        buf[2] = TAG_FLOAT;
        buf[3] = TAG_STR;
        let mut pos = 4;
        pos += 42i32.log_encode(&mut buf[pos..]);
        pos += 3.14f64.log_encode(&mut buf[pos..]);
        pos += "hello".log_encode(&mut buf[pos..]);

        let decoded = decode_args(&buf[..pos]);
        assert_eq!(decoded.len(), 3);
        assert!(matches!(decoded[0], DecodedArg::Int(42)));
        assert!(matches!(decoded[1], DecodedArg::Float(v) if (v - 3.14f64).abs() < 0.001));
        assert!(matches!(decoded[2], DecodedArg::Str(ref s) if s == "hello"));
    }

    #[test]
    fn encode_formatted_roundtrip() {
        let formatted = "debug: Vec [1, 2, 3]";
        let mut buf = [0u8; 64];
        let wrote = encode_formatted(&mut buf, formatted);
        assert_eq!(wrote, 4 + formatted.len());
        let mut full_buf = [0u8; 64];
        full_buf[0] = 1;
        full_buf[1] = TAG_FORMATTED;
        full_buf[2..2 + wrote].copy_from_slice(&buf[..wrote]);
        let decoded = decode_args(&full_buf[..2 + wrote]);
        assert_eq!(decoded.len(), 1);
        assert!(matches!(decoded[0], DecodedArg::Formatted(ref s) if s == formatted));
    }

    #[test]
    fn decode_empty_payload() {
        assert!(decode_args(&[]).is_empty());
        assert!(decode_args(&[0]).is_empty());
    }

    #[test]
    fn format_with_args_simple() {
        let args = vec![DecodedArg::Int(42), DecodedArg::Float(3.14)];
        let result = format_with_args("x: {}, y: {}", &args);
        assert!(result.contains("x: 42"));
        assert!(result.contains("y: 3.14"));
    }

    #[test]
    fn format_with_args_fewer_args_than_placeholders() {
        let args = vec![DecodedArg::Int(1)];
        let result = format_with_args("a: {}, b: {}", &args);
        assert!(result.contains("a: 1"));
    }
}
