use std::fmt;

// ── Schema bytecode opcodes ────────────────────────────────────────────────
// Each opcode is one byte: upper 4 bits = type category, lower 4 bits = variant/size.

// Primitives
pub const OP_UNIT: u8 = 0x00;
pub const OP_SIGNED_INT: u8 = 0x10;
pub const OP_UNSIGNED_INT: u8 = 0x20;
pub const OP_FLOAT: u8 = 0x30;
pub const OP_STR: u8 = 0x40;
pub const OP_CHAR: u8 = 0x50;
pub const OP_BOOL: u8 = 0x60;

// Compounds (recursive sub-schemas follow)
pub const OP_SEQ: u8 = 0x80;
pub const OP_TUPLE: u8 = 0x90;
pub const OP_STRUCT: u8 = 0xA0;
pub const OP_ENUM: u8 = 0xB0;

// Extended
pub const OP_USER_DEFINED: u8 = 0xF0;

pub const CAT_MASK: u8 = 0xF0;
pub const SIZE_MASK: u8 = 0x0F;

pub const fn op_unit() -> &'static [u8] {
    &[OP_UNIT]
}

pub const fn op_signed_int(k: u8) -> u8 {
    OP_SIGNED_INT | (k & SIZE_MASK)
}

pub const fn op_unsigned_int(k: u8) -> u8 {
    OP_UNSIGNED_INT | (k & SIZE_MASK)
}

pub const fn op_float(k: u8) -> u8 {
    OP_FLOAT | (k & SIZE_MASK)
}

pub const fn op_str(k: u8) -> u8 {
    OP_STR | (k & SIZE_MASK)
}

pub const fn op_bool() -> u8 {
    OP_BOOL
}

pub const fn op_char() -> u8 {
    OP_CHAR
}

#[inline]
pub fn size_for_k(k: u8) -> usize {
    match k {
        1 => 1,
        2 => 2,
        3 => 4,
        4 => 8,
        5 => 16,
        _ => 0,
    }
}

#[inline]
fn read_unsigned_ne(buf: &[u8], k: u8) -> u64 {
    let size = size_for_k(k);
    match size {
        1 => buf.first().copied().unwrap_or(0) as u64,
        2 => u16::from_ne_bytes([buf[0], buf[1]]) as u64,
        4 => u32::from_ne_bytes([buf[0], buf[1], buf[2], buf[3]]) as u64,
        8 => u64::from_ne_bytes([
            buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7],
        ]),
        _ => 0,
    }
}

#[inline]
fn read_signed_ne(buf: &[u8], k: u8) -> i64 {
    let size = size_for_k(k);
    match size {
        1 => buf.first().copied().unwrap_or(0) as i8 as i64,
        2 => i16::from_ne_bytes([buf[0], buf[1]]) as i64,
        4 => i32::from_ne_bytes([buf[0], buf[1], buf[2], buf[3]]) as i64,
        8 => i64::from_ne_bytes([
            buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7],
        ]),
        _ => 0,
    }
}

// ── Encode trait ───────────────────────────────────────────────────────────

pub trait Encode {
    const SCHEMA: &'static [u8];
    fn encode_to(&self, buf: &mut [u8]) -> usize;
    fn max_encoded_size(&self) -> usize;
}

/// Const helper: returns the schema for any `Encode` implementor.
/// Used by macros to get `SCHEMA` from an expression (not a type).
pub const fn schema_of<T: Encode + ?Sized>(_: &T) -> &'static [u8] {
    <T as Encode>::SCHEMA
}

/// Returns the schema length for an `Encode` implementor.
/// Used by macros to compute total encoding size.
#[inline]
pub fn schema_len<T: Encode + ?Sized>(_: &T) -> usize {
    T::SCHEMA.len()
}

pub trait HasStringTable {
    const STRING_TABLE: &'static [u8];
}

// ── UserFormatter ──────────────────────────────────────────────────────────

pub struct UserFormatter {
    pub format: fn(payload: &[u8], f: &mut dyn fmt::Write) -> (usize, fmt::Result),
}

// ── Encode impls for primitives ────────────────────────────────────────────

macro_rules! impl_encode_signed_int {
    ($ty:ty, $k:expr) => {
        impl Encode for $ty {
            const SCHEMA: &'static [u8] = &[op_signed_int($k)];

            fn encode_to(&self, buf: &mut [u8]) -> usize {
                let size = size_for_k($k);
                let val: i128 = *self as i128;
                let bytes = val.to_ne_bytes();
                buf[..size].copy_from_slice(&bytes[..size]);
                size
            }

            fn max_encoded_size(&self) -> usize {
                size_for_k($k)
            }
        }
    };
}

macro_rules! impl_encode_unsigned_int {
    ($ty:ty, $k:expr) => {
        impl Encode for $ty {
            const SCHEMA: &'static [u8] = &[op_unsigned_int($k)];

            fn encode_to(&self, buf: &mut [u8]) -> usize {
                let size = size_for_k($k);
                let val: u128 = *self as u128;
                let bytes = val.to_ne_bytes();
                buf[..size].copy_from_slice(&bytes[..size]);
                size
            }

            fn max_encoded_size(&self) -> usize {
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
    const SCHEMA: &'static [u8] = &[op_unsigned_int(if std::mem::size_of::<usize>() == 4 {
        3
    } else {
        4
    })];

    fn encode_to(&self, buf: &mut [u8]) -> usize {
        (*self as u64).encode_to(buf)
    }

    fn max_encoded_size(&self) -> usize {
        8
    }
}

impl Encode for f32 {
    const SCHEMA: &'static [u8] = &[op_float(3)];

    fn encode_to(&self, buf: &mut [u8]) -> usize {
        buf[..4].copy_from_slice(&self.to_ne_bytes());
        4
    }

    fn max_encoded_size(&self) -> usize {
        4
    }
}

impl Encode for f64 {
    const SCHEMA: &'static [u8] = &[op_float(4)];

    fn encode_to(&self, buf: &mut [u8]) -> usize {
        buf[..8].copy_from_slice(&self.to_ne_bytes());
        8
    }

    fn max_encoded_size(&self) -> usize {
        8
    }
}

impl Encode for bool {
    const SCHEMA: &'static [u8] = &[op_bool()];

    fn encode_to(&self, buf: &mut [u8]) -> usize {
        buf[0] = *self as u8;
        1
    }

    fn max_encoded_size(&self) -> usize {
        1
    }
}

impl Encode for char {
    const SCHEMA: &'static [u8] = &[op_char()];

    fn encode_to(&self, buf: &mut [u8]) -> usize {
        buf[..4].copy_from_slice(&(*self as u32).to_ne_bytes());
        4
    }

    fn max_encoded_size(&self) -> usize {
        4
    }
}

impl Encode for () {
    const SCHEMA: &'static [u8] = &[OP_UNIT];

    fn encode_to(&self, _buf: &mut [u8]) -> usize {
        0
    }

    fn max_encoded_size(&self) -> usize {
        0
    }
}

impl Encode for &str {
    const SCHEMA: &'static [u8] = &[op_str(2)];

    fn encode_to(&self, buf: &mut [u8]) -> usize {
        let bytes = self.as_bytes();
        let len = bytes.len().min(u16::MAX as usize) as u16;
        buf[..2].copy_from_slice(&len.to_ne_bytes());
        let copy_len = len as usize;
        buf[2..2 + copy_len].copy_from_slice(&bytes[..copy_len]);
        2 + copy_len
    }

    fn max_encoded_size(&self) -> usize {
        2 + self.len()
    }
}

impl Encode for String {
    const SCHEMA: &'static [u8] = <&str as Encode>::SCHEMA;

    fn encode_to(&self, buf: &mut [u8]) -> usize {
        self.as_str().encode_to(buf)
    }

    fn max_encoded_size(&self) -> usize {
        2 + self.len()
    }
}

// ── DisplayArg / DebugArg wrappers ─────────────────────────────────────────

pub struct DisplayArg<T: fmt::Display>(pub T);

impl<T: fmt::Display> Encode for DisplayArg<T> {
    const SCHEMA: &'static [u8] = <&str as Encode>::SCHEMA;

    fn encode_to(&self, buf: &mut [u8]) -> usize {
        self.0.to_string().as_str().encode_to(buf)
    }

    fn max_encoded_size(&self) -> usize {
        512
    }
}

pub struct DebugArg<T: fmt::Debug>(pub T);

impl<T: fmt::Debug> Encode for DebugArg<T> {
    const SCHEMA: &'static [u8] = <&str as Encode>::SCHEMA;

    fn encode_to(&self, buf: &mut [u8]) -> usize {
        format!("{:?}", self.0).as_str().encode_to(buf)
    }

    fn max_encoded_size(&self) -> usize {
        512
    }
}

// ── Schema measurement ─────────────────────────────────────────────────────

pub fn measure_schema(schema: &[u8]) -> usize {
    if schema.is_empty() {
        return 0;
    }
    let op = schema[0];
    let cat = op & CAT_MASK;
    let k = op & SIZE_MASK;
    let mut pos = 1;

    match cat {
        OP_UNIT | OP_SIGNED_INT | OP_UNSIGNED_INT | OP_FLOAT | OP_STR | OP_CHAR | OP_BOOL => pos,
        OP_USER_DEFINED => pos + 1,
        OP_SEQ => pos + measure_schema(&schema[pos..]),
        OP_TUPLE if k == 0 => {
            let n = u16::from_ne_bytes([schema[pos], schema[pos + 1]]) as usize;
            pos += 2;
            for _ in 0..n {
                pos += measure_schema(&schema[pos..]);
            }
            pos
        }
        OP_TUPLE => {
            for _ in 0..k {
                pos += measure_schema(&schema[pos..]);
            }
            pos
        }
        OP_STRUCT if k == 0 => {
            let n = u16::from_ne_bytes([schema[pos], schema[pos + 1]]) as usize;
            pos += 3; // n:u16 + st_idx:u8
            for _ in 0..n {
                pos += 2; // st_off:u16
                pos += measure_schema(&schema[pos..]);
            }
            pos
        }
        OP_STRUCT => {
            pos += 1; // st_idx:u8
            for _ in 0..k {
                pos += 2; // st_off:u16
                pos += measure_schema(&schema[pos..]);
            }
            pos
        }
        OP_ENUM if k == 0 => {
            let n = u16::from_ne_bytes([schema[pos], schema[pos + 1]]) as usize;
            pos += 4; // n:u16 + vdx_k:u8 + st_idx:u8
            for _ in 0..n {
                pos += 2; // st_off:u16
                pos += measure_schema(&schema[pos..]);
            }
            pos
        }
        OP_ENUM => {
            pos += 2; // vdx_k:u8 + st_idx:u8
            for _ in 0..k {
                pos += 2; // st_off:u16
                pos += measure_schema(&schema[pos..]);
            }
            pos
        }
        _ => pos,
    }
}

// ── Recursive zero-copy format_payload ─────────────────────────────────────

fn read_str_from_table<'a>(st_idx: u8, st_off: u16, string_tables: &[&'a [u8]]) -> &'a str {
    let table = string_tables.get(st_idx as usize).copied().unwrap_or(b"");
    let start = st_off as usize;
    if start >= table.len() {
        return "";
    }
    let end = table[start..]
        .iter()
        .position(|&b| b == 0)
        .map(|p| start + p)
        .unwrap_or(table.len());
    std::str::from_utf8(&table[start..end]).unwrap_or("")
}

pub fn format_payload(
    schema: &[u8],
    payload: &[u8],
    string_tables: &[&[u8]],
    user_formatters: &[UserFormatter],
    w: &mut dyn fmt::Write,
) -> usize {
    if schema.is_empty() {
        return 0;
    }
    let op = schema[0];
    let cat = op & CAT_MASK;
    let k = op & SIZE_MASK;

    match cat {
        OP_UNIT => {
            let _ = write!(w, "()");
            0
        }
        OP_SIGNED_INT => {
            let size = size_for_k(k);
            if payload.len() < size {
                return 0;
            }
            let val = read_signed_ne(payload, k);
            let _ = write!(w, "{val}");
            size
        }
        OP_UNSIGNED_INT => {
            let size = size_for_k(k);
            if payload.len() < size {
                return 0;
            }
            let val = read_unsigned_ne(payload, k);
            let _ = write!(w, "{val}");
            size
        }
        OP_FLOAT => {
            let size = size_for_k(k);
            if payload.len() < size {
                return 0;
            }
            match k {
                3 => {
                    let v = f32::from_ne_bytes([payload[0], payload[1], payload[2], payload[3]]);
                    let _ = write!(w, "{v}");
                }
                4 => {
                    let v = f64::from_ne_bytes([
                        payload[0], payload[1], payload[2], payload[3], payload[4], payload[5],
                        payload[6], payload[7],
                    ]);
                    let _ = write!(w, "{v}");
                }
                _ => {}
            }
            size
        }
        OP_STR => {
            let count_size = size_for_k(k);
            if payload.len() < count_size {
                return 0;
            }
            let len = read_unsigned_ne(payload, k) as usize;
            if payload.len() < count_size + len {
                return count_size;
            }
            let s = std::str::from_utf8(&payload[count_size..count_size + len]).unwrap_or("");
            let _ = write!(w, "{s}");
            count_size + len
        }
        OP_CHAR => {
            if payload.len() < 4 {
                return 0;
            }
            let code = u32::from_ne_bytes([payload[0], payload[1], payload[2], payload[3]]);
            if let Some(c) = char::from_u32(code) {
                let _ = write!(w, "{c}");
            }
            4
        }
        OP_BOOL => {
            if payload.is_empty() {
                return 0;
            }
            let _ = write!(w, "{}", payload[0] != 0);
            1
        }
        OP_SEQ => {
            let count_size = size_for_k(k);
            if payload.len() < count_size {
                return 0;
            }
            let count = read_unsigned_ne(payload, k) as usize;
            let elem_schema_len = measure_schema(&schema[1..]);
            let elem_schema = &schema[1..1 + elem_schema_len];
            let mut pos = count_size;
            let _ = write!(w, "[");
            for i in 0..count {
                if i > 0 {
                    let _ = write!(w, ", ");
                }
                let used = format_payload(
                    elem_schema,
                    &payload[pos..],
                    string_tables,
                    user_formatters,
                    w,
                );
                pos += used;
            }
            let _ = write!(w, "]");
            pos
        }
        OP_TUPLE => {
            let (n, schema_start) = if k == 0 {
                let n = u16::from_ne_bytes([schema[1], schema[2]]) as usize;
                (n, 3)
            } else {
                (k as usize, 1)
            };
            let mut spos = schema_start;
            let mut pos = 0;
            let _ = write!(w, "(");
            for i in 0..n {
                if i > 0 {
                    let _ = write!(w, ", ");
                }
                let elem_schema_len = measure_schema(&schema[spos..]);
                let used = format_payload(
                    &schema[spos..spos + elem_schema_len],
                    &payload[pos..],
                    string_tables,
                    user_formatters,
                    w,
                );
                pos += used;
                spos += elem_schema_len;
            }
            let _ = write!(w, ")");
            pos
        }
        OP_STRUCT => {
            let (n, schema_start) = if k == 0 {
                let n = u16::from_ne_bytes([schema[1], schema[2]]) as usize;
                (n, 3)
            } else {
                (k as usize, 1)
            };
            let st_idx = schema[schema_start];
            let mut spos = schema_start + 1;
            let mut pos = 0;
            let _ = write!(w, "{{");
            for i in 0..n {
                if i > 0 {
                    let _ = write!(w, ", ");
                }
                let st_off = u16::from_ne_bytes([schema[spos], schema[spos + 1]]);
                spos += 2;
                let field_name = read_str_from_table(st_idx, st_off, string_tables);
                let field_schema_len = measure_schema(&schema[spos..]);
                let _ = write!(w, "{field_name}: ");
                let used = format_payload(
                    &schema[spos..spos + field_schema_len],
                    &payload[pos..],
                    string_tables,
                    user_formatters,
                    w,
                );
                pos += used;
                spos += field_schema_len;
            }
            let _ = write!(w, "}}");
            pos
        }
        OP_ENUM => {
            let (n, schema_start) = if k == 0 {
                let n = u16::from_ne_bytes([schema[1], schema[2]]) as usize;
                (n, 3)
            } else {
                (k as usize, 1)
            };
            let vdx_k = schema[schema_start];
            let st_idx = schema[schema_start + 1];
            let mut spos = schema_start + 2;
            let var_count_size = size_for_k(vdx_k);
            if payload.len() < var_count_size {
                return 0;
            }
            let var_idx = read_unsigned_ne(payload, vdx_k) as usize;
            let mut pos = var_count_size;

            let mut found_name = "";
            let mut found_schema_start = 0usize;
            let mut found_schema_end = 0usize;

            for i in 0..n {
                let st_off = u16::from_ne_bytes([schema[spos], schema[spos + 1]]);
                spos += 2;
                let field_schema_len = measure_schema(&schema[spos..]);
                if i == var_idx {
                    found_name = read_str_from_table(st_idx, st_off, string_tables);
                    found_schema_start = spos;
                    found_schema_end = spos + field_schema_len;
                }
                spos += field_schema_len;
            }

            let var_schema = if found_schema_end > found_schema_start {
                &schema[found_schema_start..found_schema_end]
            } else {
                &[]
            };

            let is_unit = var_schema.len() == 1 && var_schema[0] == OP_UNIT;
            if var_schema.is_empty() || is_unit {
                let _ = write!(w, "{found_name}");
            } else {
                let _ = write!(w, "{found_name}(");
                let used = format_payload(
                    var_schema,
                    &payload[pos..],
                    string_tables,
                    user_formatters,
                    w,
                );
                pos += used;
                let _ = write!(w, ")");
            }
            pos
        }
        OP_USER_DEFINED => {
            if schema.len() < 2 {
                return 0;
            }
            let idx = schema[1] as usize;
            if let Some(uf) = user_formatters.get(idx) {
                let (used, res) = (uf.format)(payload, w);
                let _ = res;
                used
            } else {
                0
            }
        }
        _ => 0,
    }
}

// ── Public API for backend ─────────────────────────────────────────────────

fn parse_schema_offsets(payload: &[u8], arg_count: usize) -> (Vec<(usize, usize)>, usize) {
    let mut schema_pos = 1usize;
    let mut schemas: Vec<(usize, usize)> = Vec::with_capacity(arg_count);
    for _ in 0..arg_count {
        if schema_pos >= payload.len() {
            break;
        }
        let schema_len = measure_schema(&payload[schema_pos..]);
        schemas.push((schema_pos, schema_len));
        schema_pos += schema_len;
    }
    (schemas, schema_pos)
}

/// Decodes args from the packed layout: count byte + all schemas + all payloads.
pub fn decode_args(payload: &[u8]) -> Vec<String> {
    if payload.is_empty() {
        return vec![];
    }
    let arg_count = payload[0] as usize;
    if arg_count == 0 || payload.is_empty() {
        return vec![];
    }

    let (schemas, mut payload_pos) = parse_schema_offsets(payload, arg_count);

    let mut results = Vec::with_capacity(arg_count);
    for (schema_start, schema_len) in &schemas {
        let schema = &payload[*schema_start..*schema_start + *schema_len];
        let mut s = String::new();
        let used = format_payload(schema, &payload[payload_pos..], &[], &[], &mut s);
        payload_pos += used;
        results.push(s);
    }
    results
}

/// Format the log body by walking format_str + inline schemas + payload.
pub fn format_body(metadata: &crate::metadata::Metadata, payload: &[u8]) -> String {
    let mut output = String::new();
    let mut remaining = metadata.format_str;
    let mut schema_idx = 0;

    if payload.is_empty() || payload[0] as usize == 0 {
        output.push_str(remaining);
        return output;
    }

    let arg_count = payload[0] as usize;
    let (schemas, mut payload_pos) = parse_schema_offsets(payload, arg_count);

    while let Some(brace) = remaining.find('{') {
        output.push_str(&remaining[..brace]);
        let after_brace = &remaining[brace + 1..];
        let Some(close) = after_brace.find('}') else {
            output.push_str(remaining);
            remaining = "";
            break;
        };
        remaining = &after_brace[close + 1..];

        if schema_idx < schemas.len() {
            let (schema_start, schema_len) = schemas[schema_idx];
            let schema = &payload[schema_start..schema_start + schema_len];
            let used = format_payload(schema, &payload[payload_pos..], &[], &[], &mut output);
            payload_pos += used;
            schema_idx += 1;
        }
    }
    output.push_str(remaining);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Encode roundtrip tests ─────────────────────────────────────────

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

    // ── Schema tests ───────────────────────────────────────────────────

    #[test]
    fn schema_i32_is_signed_int_3() {
        assert_eq!(<i32 as Encode>::SCHEMA, &[op_signed_int(3)]);
    }

    #[test]
    fn schema_f64_is_float_4() {
        assert_eq!(<f64 as Encode>::SCHEMA, &[op_float(4)]);
    }

    #[test]
    fn schema_bool() {
        assert_eq!(<bool as Encode>::SCHEMA, &[op_bool()]);
    }

    #[test]
    fn schema_str() {
        assert_eq!(<&str as Encode>::SCHEMA, &[op_str(2)]);
    }

    // ── measure_schema tests ───────────────────────────────────────────

    #[test]
    fn measure_schema_primitive() {
        assert_eq!(measure_schema(&[op_signed_int(3)]), 1);
        assert_eq!(measure_schema(&[op_float(4)]), 1);
        assert_eq!(measure_schema(&[op_bool()]), 1);
    }

    #[test]
    fn measure_schema_seq() {
        let schema = [OP_SEQ | 2, op_signed_int(3)]; // SEQ(k=2) + i32
        assert_eq!(measure_schema(&schema), 2);
    }

    #[test]
    fn measure_schema_tuple_inline() {
        let schema = [OP_TUPLE | 2, op_signed_int(4), op_float(4)]; // TUPLE(2): i64, f64
        assert_eq!(measure_schema(&schema), 3);
    }

    #[test]
    fn measure_schema_empty() {
        assert_eq!(measure_schema(&[]), 0);
    }

    // ── format_payload tests ───────────────────────────────────────────

    fn format_one(schema: &[u8], payload: &[u8]) -> String {
        let mut s = String::new();
        format_payload(schema, payload, &[], &[], &mut s);
        s
    }

    #[test]
    fn format_i32() {
        let mut buf = [0u8; 4];
        42i32.encode_to(&mut buf);
        let result = format_one(<i32 as Encode>::SCHEMA, &buf);
        assert_eq!(result, "42");
    }

    #[test]
    fn format_i64_negative() {
        let mut buf = [0u8; 8];
        (-123i64).encode_to(&mut buf);
        let result = format_one(<i64 as Encode>::SCHEMA, &buf);
        assert_eq!(result, "-123");
    }

    #[test]
    fn format_u32() {
        let mut buf = [0u8; 4];
        100u32.encode_to(&mut buf);
        let result = format_one(<u32 as Encode>::SCHEMA, &buf);
        assert_eq!(result, "100");
    }

    #[test]
    fn format_f64() {
        let mut buf = [0u8; 8];
        3.14f64.encode_to(&mut buf);
        let result = format_one(<f64 as Encode>::SCHEMA, &buf);
        assert!(result.contains("3.14"));
    }

    #[test]
    fn format_str() {
        let mut buf = [0u8; 32];
        let wrote = "hello".encode_to(&mut buf);
        let result = format_one(<&str as Encode>::SCHEMA, &buf[..wrote]);
        assert_eq!(result, "hello");
    }

    #[test]
    fn format_bool() {
        let result = format_one(<bool as Encode>::SCHEMA, &[1]);
        assert_eq!(result, "true");
        let result = format_one(<bool as Encode>::SCHEMA, &[0]);
        assert_eq!(result, "false");
    }

    #[test]
    fn format_char() {
        let mut buf = [0u8; 4];
        'X'.encode_to(&mut buf);
        let result = format_one(<char as Encode>::SCHEMA, &buf);
        assert_eq!(result, "X");
    }

    #[test]
    fn format_unit() {
        let result = format_one(<() as Encode>::SCHEMA, &[]);
        assert_eq!(result, "()");
    }

    #[test]
    fn format_seq_i32() {
        let mut buf = [0u8; 64];
        // Manual SEQ(k=2) of i32s: count(2 as u16 LE) + 10_i32 + 20_i32
        let count: u16 = 2;
        buf[..2].copy_from_slice(&count.to_ne_bytes());
        let mut pos = 2;
        pos += 10i32.encode_to(&mut buf[pos..]);
        pos += 20i32.encode_to(&mut buf[pos..]);
        let schema = [OP_SEQ | 2, op_signed_int(3)];
        let result = format_one(&schema, &buf[..pos]);
        assert_eq!(result, "[10, 20]");
    }

    #[test]
    fn format_tuple_two() {
        let mut buf = [0u8; 64];
        let mut pos = 0;
        pos += 5i64.encode_to(&mut buf[pos..]);
        pos += 2.5f64.encode_to(&mut buf[pos..]);
        let schema = [OP_TUPLE | 2, op_signed_int(4), op_float(4)];
        let result = format_one(&schema, &buf[..pos]);
        assert_eq!(result, "(5, 2.5)");
    }

    #[test]
    fn format_struct_two_fields() {
        let mut buf = [0u8; 64];
        let mut pos = 0;
        pos += 1i32.encode_to(&mut buf[pos..]);
        pos += "xyz".encode_to(&mut buf[pos..]);
        let st: &[&[u8]] = &[b"x\0y\0"];
        // STRUCT(2): st_idx=0, [st_off=0, i32_schema], [st_off=2, str_schema]
        let schema = [
            OP_STRUCT | 2,
            0u8, // st_idx
            0u8,
            0u8, // st_off = 0 (field "x")
            op_signed_int(3),
            2u8,
            0u8, // st_off = 2 (field "y")
            op_str(2),
        ];
        let mut s = String::new();
        format_payload(&schema, &buf[..pos], st, &[], &mut s);
        assert_eq!(s, "{x: 1, y: xyz}");
    }

    #[test]
    fn format_enum_unit_variant() {
        let schema = [
            OP_ENUM | 2,
            1u8, // vdx_k (u8 variant index)
            0u8, // st_idx
            0u8,
            0u8, // st_off = 0 → "None"
            OP_UNIT,
            5u8,
            0u8, // st_off = 5 → "Some"
            op_signed_int(3),
        ];
        let st: &[u8] = b"None\0Some\0";
        // Variant 0 (None)
        let payload = [0u8]; // variant index 0 (u8)
        let mut s = String::new();
        format_payload(&schema, &payload, &[st], &[], &mut s);
        assert_eq!(s, "None");

        // Variant 1 (Some)
        let mut payload = [0u8; 8];
        payload[0] = 1; // variant index 1
        payload[1..5].copy_from_slice(&42i32.to_ne_bytes());
        let mut s = String::new();
        format_payload(&schema, &payload, &[st], &[], &mut s);
        assert_eq!(s, "Some(42)");
    }

    #[test]
    fn format_empty_schema() {
        let result = format_one(&[], &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn format_large_i32() {
        // Verify read_signed_ne uses correct byte count (was using k=3→3 bytes, should be 4)
        let val: i32 = 0x7FFF_FFFF; // max i32: 2147483647
        let mut buf = [0u8; 4];
        val.encode_to(&mut buf);
        let result = format_one(<i32 as Encode>::SCHEMA, &buf);
        assert_eq!(result, "2147483647");
    }

    #[test]
    fn format_large_u32() {
        let val: u32 = 0xFFFF_FFFF; // max u32: 4294967295
        let mut buf = [0u8; 4];
        val.encode_to(&mut buf);
        let result = format_one(<u32 as Encode>::SCHEMA, &buf);
        assert_eq!(result, "4294967295");
    }

    #[test]
    fn format_large_i64() {
        let val: i64 = 0x7FFF_FFFF_FFFF_FFFF; // max i64
        let mut buf = [0u8; 8];
        val.encode_to(&mut buf);
        let result = format_one(<i64 as Encode>::SCHEMA, &buf);
        assert_eq!(result, "9223372036854775807");
    }

    // ── format_body tests ──────────────────────────────────────────────

    #[test]
    fn format_body_simple() {
        let metadata = crate::metadata::Metadata::new(
            crate::level::LogLevel::Info,
            "x: {}, y: {}",
            "test.rs",
            1,
            "test",
        );
        // Payload: count(2) + schema_i32 + schema_f64 + payload_i32 + payload_f64
        let mut payload = vec![0u8; 1 + 1 + 1 + 4 + 8]; // count + 2 schemas + data
        payload[0] = 2; // arg_count
        payload[1] = op_signed_int(3); // i32 schema
        payload[2] = op_float(4); // f64 schema
        let data_off = 3;
        42i32.encode_to(&mut payload[data_off..]);
        3.14f64.encode_to(&mut payload[data_off + 4..]);
        let result = format_body(&metadata, &payload);
        assert!(result.contains("x: 42"), "got: {result}");
        assert!(result.contains("y: 3.14"), "got: {result}");
    }

    #[test]
    fn format_body_no_args() {
        let metadata = crate::metadata::Metadata::new(
            crate::level::LogLevel::Info,
            "no args",
            "test.rs",
            1,
            "test",
        );
        let result = format_body(&metadata, &[0]);
        assert_eq!(result, "no args");
    }

    #[test]
    fn format_body_fewer_args_than_placeholders() {
        let metadata = crate::metadata::Metadata::new(
            crate::level::LogLevel::Info,
            "a: {}, b: {}",
            "test.rs",
            1,
            "test",
        );
        // Payload: count(1) + schema_i32 + data_i32
        let mut payload = vec![0u8; 1 + 1 + 4];
        payload[0] = 1; // arg_count
        payload[1] = op_signed_int(3);
        42i32.encode_to(&mut payload[2..]);
        let result = format_body(&metadata, &payload);
        assert!(result.contains("a: 42"));
    }

    #[test]
    fn format_body_with_debug_spec() {
        let metadata = crate::metadata::Metadata::new(
            crate::level::LogLevel::Info,
            "val: {:?}",
            "test.rs",
            1,
            "test",
        );
        let mut payload = vec![0u8; 1 + 1 + 4];
        payload[0] = 1;
        payload[1] = op_signed_int(3);
        99i32.encode_to(&mut payload[2..]);
        let result = format_body(&metadata, &payload);
        assert!(result.contains("val: 99"), "got: {result}");
    }

    // ── measure_schema compound tests ──────────────────────────────────

    #[test]
    fn measure_schema_struct_inline() {
        // STRUCT(2): [opcode] [st_idx] [st_off:2][i32_schema] [st_off:2][f64_schema]
        let schema = [
            OP_STRUCT | 2,
            0u8,
            0u8,
            0u8,
            op_signed_int(3),
            2u8,
            0u8,
            op_float(4),
        ];
        assert_eq!(measure_schema(&schema), 1 + 1 + 2 + 1 + 2 + 1);
    }

    #[test]
    fn measure_schema_struct_u16() {
        // STRUCT(0: u16 count=2): [opcode] [n:2] [st_idx] [st_off:2][i32] [st_off:2][f64]
        let schema = [
            OP_STRUCT, // k=0 → u16 count follows
            2u8,
            0u8, // n = 2 as u16 LE
            0u8, // st_idx
            0u8,
            0u8, // st_off = 0
            op_signed_int(3),
            2u8,
            0u8, // st_off = 2
            op_float(4),
        ];
        assert_eq!(measure_schema(&schema), 1 + 2 + 1 + 2 + 1 + 2 + 1);
    }

    #[test]
    fn measure_schema_enum_inline() {
        // ENUM(2): [opcode] [vdx_k] [st_idx] [st_off:2][unit] [st_off:2][i32]
        let schema = [
            OP_ENUM | 2,
            1u8, // vdx_k = 1 (u8 discriminant)
            0u8, // st_idx
            0u8,
            0u8, // variant 0 "None" → unit
            OP_UNIT,
            5u8,
            0u8, // variant 1 "Some" → i32
            op_signed_int(3),
        ];
        assert_eq!(measure_schema(&schema), 1 + 1 + 1 + 2 + 1 + 2 + 1);
    }

    #[test]
    fn measure_schema_enum_u16() {
        // ENUM(0: u16 count=2): [opcode] [n:2] [vdx_k] [st_idx] [st_off:2][unit] [st_off:2][i32]
        let schema = [
            OP_ENUM, // k=0 → u16 count follows
            2u8,
            0u8, // n = 2 as u16 LE
            1u8, // vdx_k = 1
            0u8, // st_idx
            0u8,
            0u8, // variant 0
            OP_UNIT,
            5u8,
            0u8, // variant 1
            op_signed_int(3),
        ];
        assert_eq!(measure_schema(&schema), 1 + 2 + 1 + 1 + 2 + 1 + 2 + 1);
    }

    #[test]
    fn measure_schema_tuple_u16() {
        // TUPLE(0: u16 count=2): [opcode] [n:2] [i64_schema] [f64_schema]
        let schema = [
            OP_TUPLE, // k=0 → u16 count follows
            2u8,
            0u8, // n = 2
            op_signed_int(4),
            op_float(4),
        ];
        assert_eq!(measure_schema(&schema), 1 + 2 + 1 + 1);
    }

    // ── format_payload compound tests ───────────────────────────────────

    #[test]
    fn format_struct_u16() {
        let mut buf = [0u8; 64];
        let mut pos = 0;
        pos += 100i32.encode_to(&mut buf[pos..]);
        pos += "abc".encode_to(&mut buf[pos..]);
        let st: &[&[u8]] = &[b"field1\0field2\0"];
        // STRUCT(k=0, n=2): [op][n:2][st_idx][off:2][i32][off:2][str]
        let schema = [
            OP_STRUCT,
            2u8,
            0u8, // n = 2
            0u8, // st_idx
            0u8,
            0u8, // field1
            op_signed_int(3),
            7u8,
            0u8, // field2
            op_str(2),
        ];
        let mut s = String::new();
        format_payload(&schema, &buf[..pos], st, &[], &mut s);
        assert_eq!(s, "{field1: 100, field2: abc}");
    }

    #[test]
    fn format_enum_u16() {
        // ENUM(k=0, n=2, vdx_k=1): [op][n:2][vdx_k][st_idx][off:2][unit][off:2][i32]
        let schema = [
            OP_ENUM,
            2u8,
            0u8, // n = 2
            1u8, // vdx_k = 1
            0u8, // st_idx
            0u8,
            0u8, // variant 0 "A"
            OP_UNIT,
            2u8,
            0u8, // variant 1 "B"
            op_signed_int(3),
        ];
        let st: &[&[u8]] = &[b"A\0B\0"];

        // Variant 0 (unit)
        let payload = [0u8]; // discriminant = 0
        let mut s = String::new();
        format_payload(&schema, &payload, st, &[], &mut s);
        assert_eq!(s, "A");

        // Variant 1 (i32=42)
        let mut payload = [0u8; 8];
        payload[0] = 1;
        payload[1..5].copy_from_slice(&42i32.to_ne_bytes());
        let mut s = String::new();
        format_payload(&schema, &payload, st, &[], &mut s);
        assert_eq!(s, "B(42)");
    }

    // ── format_payload edge case tests ──────────────────────────────────

    #[test]
    fn format_truncated_payload() {
        // I32 needs 4 bytes, but give only 1
        let schema = [op_signed_int(3)];
        let result = format_one(&schema, &[0u8; 1]);
        assert!(result.is_empty());

        // Str with u16 len needs 2 + len, give only 1
        let schema = [op_str(2)];
        let result = format_one(&schema, &[0u8; 1]);
        assert!(result.is_empty());
    }

    #[test]
    fn format_zero_count_seq() {
        // SEQ(k=2) of i32s with count=0
        let schema = [OP_SEQ | 2, op_signed_int(3)];
        let payload = [0u8, 0u8]; // count = 0 as u16 LE
        let result = format_one(&schema, &payload);
        assert_eq!(result, "[]");
    }

    #[test]
    fn format_empty_payload_nonempty_schema() {
        let result = format_one(&[op_signed_int(3)], &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn format_unknown_opcode() {
        let result = format_one(&[0xCCu8], &[1u8; 8]);
        assert!(result.is_empty());
    }

    // ── decode_args tests ───────────────────────────────────────────────

    #[test]
    fn decode_args_two_ints() {
        let mut buf = [0u8; 32];
        buf[0] = 2; // arg_count
        buf[1] = op_signed_int(3);
        buf[2] = op_float(4);
        let mut pos = 3;
        pos += 42i32.encode_to(&mut buf[pos..]);
        3.14f64.encode_to(&mut buf[pos..]);
        let decoded = decode_args(&buf);
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0], "42");
        assert!(decoded[1].contains("3.14"));
    }

    #[test]
    fn decode_args_empty() {
        assert!(decode_args(&[]).is_empty());
    }

    #[test]
    fn decode_args_zero_arg_count() {
        let buf = [0u8];
        assert!(decode_args(&buf).is_empty());
    }

    #[test]
    fn decode_args_truncated_schema() {
        // count=2 but only 1 schema byte available → only 1 schema parsed
        let buf = [2u8, 0x13]; // count=2, i32 schema, no payload
        let decoded = decode_args(&buf);
        assert_eq!(decoded.len(), 1, "expected 1 schema, got {:?}", decoded);
    }

    // ── format_body edge case tests ─────────────────────────────────────

    #[test]
    fn format_body_extra_placeholders() {
        let metadata = crate::metadata::Metadata::new(
            crate::level::LogLevel::Info,
            "a: {}, b: {}",
            "test.rs",
            1,
            "test",
        );
        // Only one arg in payload
        let mut payload = vec![0u8; 1 + 1 + 4];
        payload[0] = 1;
        payload[1] = op_signed_int(3);
        5i32.encode_to(&mut payload[2..]);
        let result = format_body(&metadata, &payload);
        assert!(result.contains("a: 5"));
        // The second placeholder is unmatched; literal "b: " appears but {} is skipped
        assert!(!result.contains("b: 5"));
    }

    #[test]
    fn format_body_empty_payload() {
        let metadata = crate::metadata::Metadata::new(
            crate::level::LogLevel::Info,
            "hello",
            "test.rs",
            1,
            "test",
        );
        let result = format_body(&metadata, &[]);
        assert_eq!(result, "hello");
    }
}
