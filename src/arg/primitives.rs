use std::fmt;

use super::traits::Encode;
use super::{
    OP_UNIT, op_bool, op_char, op_float, op_signed_int, op_str, op_unsigned_int, size_for_k,
};

// ── Encode impls for primitives ────────────────────────────────────────────

macro_rules! impl_encode_signed_int {
    ($ty:ty, $k:expr) => {
        impl Encode for $ty {
            fn schema() -> &'static [u8] {
                static S: &[u8] = &[op_signed_int($k)];
                S
            }

            fn encode_to(&self, buf: &mut [u8]) -> usize {
                let size = size_for_k($k);
                let val: i128 = *self as i128;
                let bytes = val.to_ne_bytes();
                buf[..size].copy_from_slice(&bytes[..size]);
                size
            }

            fn encoded_size(&self) -> usize {
                size_for_k($k)
            }
        }
    };
}

macro_rules! impl_encode_unsigned_int {
    ($ty:ty, $k:expr) => {
        impl Encode for $ty {
            fn schema() -> &'static [u8] {
                static S: &[u8] = &[op_unsigned_int($k)];
                S
            }

            fn encode_to(&self, buf: &mut [u8]) -> usize {
                let size = size_for_k($k);
                let val: u128 = *self as u128;
                let bytes = val.to_ne_bytes();
                buf[..size].copy_from_slice(&bytes[..size]);
                size
            }

            fn encoded_size(&self) -> usize {
                size_for_k($k)
            }
        }
    };
}

impl_encode_signed_int!(i8, 1);
impl_encode_signed_int!(i16, 2);
impl_encode_signed_int!(i32, 3);
impl_encode_signed_int!(i64, 4);
impl_encode_signed_int!(i128, 5);

impl_encode_unsigned_int!(u8, 1);
impl_encode_unsigned_int!(u16, 2);
impl_encode_unsigned_int!(u32, 3);
impl_encode_unsigned_int!(u64, 4);
impl_encode_unsigned_int!(u128, 5);

impl Encode for usize {
    fn schema() -> &'static [u8] {
        static S3: &[u8] = &[op_unsigned_int(3)];
        static S4: &[u8] = &[op_unsigned_int(4)];
        if std::mem::size_of::<usize>() == 4 {
            S3
        } else {
            S4
        }
    }

    fn encode_to(&self, buf: &mut [u8]) -> usize {
        (*self as u64).encode_to(buf)
    }

    fn encoded_size(&self) -> usize {
        8
    }
}

impl Encode for f32 {
    fn schema() -> &'static [u8] {
        static S: &[u8] = &[op_float(3)];
        S
    }

    fn encode_to(&self, buf: &mut [u8]) -> usize {
        buf[..4].copy_from_slice(&self.to_ne_bytes());
        4
    }

    fn encoded_size(&self) -> usize {
        4
    }
}

impl Encode for f64 {
    fn schema() -> &'static [u8] {
        static S: &[u8] = &[op_float(4)];
        S
    }

    fn encode_to(&self, buf: &mut [u8]) -> usize {
        buf[..8].copy_from_slice(&self.to_ne_bytes());
        8
    }

    fn encoded_size(&self) -> usize {
        8
    }
}

impl Encode for bool {
    fn schema() -> &'static [u8] {
        static S: &[u8] = &[op_bool()];
        S
    }

    fn encode_to(&self, buf: &mut [u8]) -> usize {
        buf[0] = *self as u8;
        1
    }

    fn encoded_size(&self) -> usize {
        1
    }
}

impl Encode for char {
    fn schema() -> &'static [u8] {
        static S: &[u8] = &[op_char()];
        S
    }

    fn encode_to(&self, buf: &mut [u8]) -> usize {
        buf[..4].copy_from_slice(&(*self as u32).to_ne_bytes());
        4
    }

    fn encoded_size(&self) -> usize {
        4
    }
}

impl Encode for () {
    fn schema() -> &'static [u8] {
        static S: &[u8] = &[OP_UNIT];
        S
    }

    fn encode_to(&self, _buf: &mut [u8]) -> usize {
        0
    }

    fn encoded_size(&self) -> usize {
        0
    }
}

impl Encode for &str {
    fn schema() -> &'static [u8] {
        static S: &[u8] = &[op_str(2)];
        S
    }

    fn encode_to(&self, buf: &mut [u8]) -> usize {
        let bytes = self.as_bytes();
        let len = bytes.len().min(u16::MAX as usize) as u16;
        buf[..2].copy_from_slice(&len.to_ne_bytes());
        let copy_len = len as usize;
        buf[2..2 + copy_len].copy_from_slice(&bytes[..copy_len]);
        2 + copy_len
    }

    fn encoded_size(&self) -> usize {
        2 + self.len()
    }
}

impl Encode for String {
    fn schema() -> &'static [u8] {
        static S: &[u8] = &[op_str(2)];
        S
    }

    fn encode_to(&self, buf: &mut [u8]) -> usize {
        self.as_str().encode_to(buf)
    }

    fn encoded_size(&self) -> usize {
        2 + self.len()
    }
}

// ── DisplayArg / DebugArg wrappers ─────────────────────────────────────────

pub struct DisplayArg<T: fmt::Display>(pub T);

impl<T: fmt::Display> Encode for DisplayArg<T> {
    fn schema() -> &'static [u8] {
        static S: &[u8] = &[op_str(2)];
        S
    }

    fn encode_to(&self, buf: &mut [u8]) -> usize {
        self.0.to_string().as_str().encode_to(buf)
    }

    fn encoded_size(&self) -> usize {
        512
    }
}

pub struct DebugArg<T: fmt::Debug>(pub T);

impl<T: fmt::Debug> Encode for DebugArg<T> {
    fn schema() -> &'static [u8] {
        static S: &[u8] = &[op_str(2)];
        S
    }

    fn encode_to(&self, buf: &mut [u8]) -> usize {
        format!("{:?}", self.0).as_str().encode_to(buf)
    }

    fn encoded_size(&self) -> usize {
        512
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_i32() {
        let val: i32 = 42;
        let mut buf = [0u8; 16];
        let wrote = val.encode_to(&mut buf);
        assert_eq!(wrote, 4);
        assert_eq!(i32::from_ne_bytes([buf[0], buf[1], buf[2], buf[3]]), 42);
    }

    #[test]
    fn encode_i64() {
        let val: i64 = -1234567890123;
        let mut buf = [0u8; 16];
        let wrote = val.encode_to(&mut buf);
        assert_eq!(wrote, 8);
        let decoded = i64::from_ne_bytes([
            buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7],
        ]);
        assert_eq!(decoded, val);
    }

    #[test]
    fn encode_f64() {
        let val: f64 = 3.1415926535;
        let mut buf = [0u8; 16];
        let wrote = val.encode_to(&mut buf);
        assert_eq!(wrote, 8);
        let decoded = f64::from_ne_bytes([
            buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7],
        ]);
        assert!((decoded - val).abs() < 1e-10);
    }

    #[test]
    fn encode_bool_true() {
        let mut buf = [0u8; 4];
        let wrote = true.encode_to(&mut buf);
        assert_eq!(wrote, 1);
        assert_eq!(buf[0], 1);
    }

    #[test]
    fn encode_bool_false() {
        let mut buf = [0u8; 4];
        let wrote = false.encode_to(&mut buf);
        assert_eq!(wrote, 1);
        assert_eq!(buf[0], 0);
    }

    #[test]
    fn encode_char() {
        let mut buf = [0u8; 4];
        let wrote = '🦀'.encode_to(&mut buf);
        assert_eq!(wrote, 4);
        let code = u32::from_ne_bytes(buf);
        assert_eq!(code, '🦀' as u32);
    }

    #[test]
    fn encode_unit() {
        let mut buf = [0u8; 4];
        let wrote = ().encode_to(&mut buf);
        assert_eq!(wrote, 0);
    }

    #[test]
    fn encode_str() {
        let val: &str = "hello world";
        let mut buf = [0u8; 32];
        let wrote = val.encode_to(&mut buf);
        assert_eq!(wrote, 2 + 11);
        let len = u16::from_ne_bytes([buf[0], buf[1]]);
        assert_eq!(len, 11);
        assert_eq!(&buf[2..13], b"hello world");
    }

    #[test]
    fn encode_string() {
        let val = String::from("test");
        let mut buf = [0u8; 32];
        let wrote = val.encode_to(&mut buf);
        assert_eq!(wrote, 6);
    }

    #[test]
    fn encode_display_arg() {
        let val = DisplayArg(42i32);
        let mut buf = [0u8; 32];
        let wrote = val.encode_to(&mut buf);
        let len = u16::from_ne_bytes([buf[0], buf[1]]);
        assert_eq!(&buf[2..2 + len as usize], b"42");
        assert!(wrote > 0);
    }

    #[test]
    fn encode_debug_arg() {
        let val = DebugArg(vec![1, 2, 3]);
        let mut buf = [0u8; 64];
        let wrote = val.encode_to(&mut buf);
        let len = u16::from_ne_bytes([buf[0], buf[1]]);
        let s = std::str::from_utf8(&buf[2..2 + len as usize]).unwrap();
        assert!(s.contains("[1, 2, 3]"));
        assert!(wrote > 0);
    }

    #[test]
    fn schema_i32_is_signed_int_3() {
        assert_eq!(<i32 as Encode>::schema(), &[op_signed_int(3)]);
    }

    #[test]
    fn schema_f64_is_float_4() {
        assert_eq!(<f64 as Encode>::schema(), &[op_float(4)]);
    }

    #[test]
    fn schema_bool() {
        assert_eq!(<bool as Encode>::schema(), &[op_bool()]);
    }

    #[test]
    fn schema_str() {
        assert_eq!(<&str as Encode>::schema(), &[op_str(2)]);
    }
}
