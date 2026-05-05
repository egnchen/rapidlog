use std::collections::HashMap;
use std::fmt;
use std::sync::LazyLock;

use crate::metadata::Metadata;

use super::traits::UserFormatter;
use super::{
    CAT_MASK, OP_BOOL, OP_CHAR, OP_ENUM, OP_FLOAT, OP_SEQ, OP_SIGNED_INT, OP_STR, OP_STRUCT,
    OP_TUPLE, OP_UNIT, OP_UNSIGNED_INT, OP_USER_DEFINED, SIZE_MASK, read_signed_ne,
    read_unsigned_ne, size_for_k,
};

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

fn parse_schema_offsets(
    schemas: &[u8],
    arg_count: usize,
    skip_count_byte: bool,
) -> Vec<(usize, usize)> {
    let mut pos = if skip_count_byte { 1 } else { 0 };
    let mut offsets: Vec<(usize, usize)> = Vec::with_capacity(arg_count);
    for _ in 0..arg_count {
        if pos >= schemas.len() {
            break;
        }
        let schema_len = measure_schema(&schemas[pos..]);
        offsets.push((pos, schema_len));
        pos += schema_len;
    }
    offsets
}

struct SchemaInfo {
    raw: &'static [u8],
    offsets: Vec<(usize, usize)>,
}

/// Returns the schemas for this call site from Metadata.
/// Format: [count: u8][concatenated schema bytes...]
type OffsetCache = parking_lot::Mutex<HashMap<usize, Vec<(usize, usize)>>>;

fn get_schemas(metadata: &Metadata) -> Option<SchemaInfo> {
    static OFFSETS: LazyLock<OffsetCache> =
        LazyLock::new(|| parking_lot::Mutex::new(HashMap::new()));

    let raw = (metadata.schema_provider)();
    if raw.is_empty() {
        return None;
    }
    let arg_count = raw[0] as usize;
    if arg_count == 0 {
        return None;
    }

    let offsets = OFFSETS
        .lock()
        .entry(raw.as_ptr() as usize)
        .or_insert_with(|| parse_schema_offsets(raw, arg_count, true))
        .clone();

    Some(SchemaInfo { raw, offsets })
}

/// Decodes args from payload using schemas from Metadata.
pub fn decode_args(metadata: &Metadata, payload: &[u8]) -> Vec<String> {
    let Some(info) = get_schemas(metadata) else {
        return vec![];
    };
    let string_tables = (metadata.string_tables_provider)();
    let user_formatters = (metadata.user_formatters_provider)();
    let mut results = Vec::with_capacity(info.offsets.len());
    let mut payload_pos = 0;
    for (schema_idx, (schema_start, schema_len)) in info.offsets.iter().enumerate() {
        let schema = &info.raw[*schema_start..*schema_start + *schema_len];
        let mut s = String::new();
        let arg_string_tables: &[&[u8]] = if schema_idx < string_tables.len() {
            // BUG #1 fix: pass only this arg's string table
            &[string_tables[schema_idx]]
        } else {
            &[]
        };
        let used = format_payload(
            schema,
            &payload[payload_pos..],
            arg_string_tables,
            user_formatters,
            &mut s,
        );
        payload_pos += used;
        results.push(s);
    }
    results
}

/// Format the log body using schemas from Metadata, raw values from payload.
pub fn format_body(metadata: &Metadata, payload: &[u8]) -> String {
    let mut output = String::new();
    let mut remaining = metadata.format_str;

    let string_tables = (metadata.string_tables_provider)();
    let user_formatters = (metadata.user_formatters_provider)();

    if let Some(info) = get_schemas(metadata) {
        let mut schema_idx = 0;
        let mut payload_pos = 0;

        while let Some(brace) = remaining.find('{') {
            output.push_str(&remaining[..brace]);
            let after_brace = &remaining[brace + 1..];
            let Some(close) = after_brace.find('}') else {
                output.push_str(remaining);
                remaining = "";
                break;
            };
            remaining = &after_brace[close + 1..];

            if schema_idx < info.offsets.len() {
                let (schema_start, schema_len) = info.offsets[schema_idx];
                let schema = &info.raw[schema_start..schema_start + schema_len];
                let arg_string_tables: &[&[u8]] = if schema_idx < string_tables.len() {
                    // BUG #1 fix: pass only this arg's string table
                    &[string_tables[schema_idx]]
                } else {
                    &[]
                };
                let used = format_payload(
                    schema,
                    &payload[payload_pos..],
                    arg_string_tables,
                    user_formatters,
                    &mut output,
                );
                payload_pos += used;
                schema_idx += 1;
            }
        }
    }

    output.push_str(remaining);
    output
}

#[cfg(test)]
mod tests {
    use super::super::*;

    fn format_one(schema: &[u8], payload: &[u8]) -> String {
        let mut s = String::new();
        format_payload(schema, payload, &[], &[], &mut s);
        s
    }

    // ── measure_schema ─────────────────────────────────────────────────

    #[test]
    fn measure_schema_primitive() {
        assert_eq!(measure_schema(&[op_signed_int(3)]), 1);
        assert_eq!(measure_schema(&[op_float(4)]), 1);
        assert_eq!(measure_schema(&[op_bool()]), 1);
    }

    #[test]
    fn measure_schema_seq() {
        let schema = [OP_SEQ | 2, op_signed_int(3)];
        assert_eq!(measure_schema(&schema), 2);
    }

    #[test]
    fn measure_schema_tuple_inline() {
        let schema = [OP_TUPLE | 2, op_signed_int(4), op_float(4)];
        assert_eq!(measure_schema(&schema), 3);
    }

    #[test]
    fn measure_schema_empty() {
        assert_eq!(measure_schema(&[]), 0);
    }

    #[test]
    fn measure_schema_struct_inline() {
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
        let schema = [
            OP_STRUCT,
            2u8,
            0u8,
            0u8,
            0u8,
            0u8,
            op_signed_int(3),
            2u8,
            0u8,
            op_float(4),
        ];
        assert_eq!(measure_schema(&schema), 1 + 2 + 1 + 2 + 1 + 2 + 1);
    }

    #[test]
    fn measure_schema_enum_inline() {
        let schema = [
            OP_ENUM | 2,
            1u8,
            0u8,
            0u8,
            0u8,
            OP_UNIT,
            5u8,
            0u8,
            op_signed_int(3),
        ];
        assert_eq!(measure_schema(&schema), 1 + 1 + 1 + 2 + 1 + 2 + 1);
    }

    #[test]
    fn measure_schema_enum_u16() {
        let schema = [
            OP_ENUM,
            2u8,
            0u8,
            1u8,
            0u8,
            0u8,
            0u8,
            OP_UNIT,
            5u8,
            0u8,
            op_signed_int(3),
        ];
        assert_eq!(measure_schema(&schema), 1 + 2 + 1 + 1 + 2 + 1 + 2 + 1);
    }

    #[test]
    fn measure_schema_tuple_u16() {
        let schema = [OP_TUPLE, 2u8, 0u8, op_signed_int(4), op_float(4)];
        assert_eq!(measure_schema(&schema), 1 + 2 + 1 + 1);
    }

    // ── format_payload primitives ──────────────────────────────────────

    #[test]
    fn format_i32() {
        let mut buf = [0u8; 4];
        42i32.encode_to(&mut buf);
        let result = format_one(<i32 as Encode>::schema(), &buf);
        assert_eq!(result, "42");
    }

    #[test]
    fn format_i64_negative() {
        let mut buf = [0u8; 8];
        (-123i64).encode_to(&mut buf);
        let result = format_one(<i64 as Encode>::schema(), &buf);
        assert_eq!(result, "-123");
    }

    #[test]
    fn format_u32() {
        let mut buf = [0u8; 4];
        100u32.encode_to(&mut buf);
        let result = format_one(<u32 as Encode>::schema(), &buf);
        assert_eq!(result, "100");
    }

    #[test]
    fn format_f64() {
        let mut buf = [0u8; 8];
        3.14f64.encode_to(&mut buf);
        let result = format_one(<f64 as Encode>::schema(), &buf);
        assert!(result.contains("3.14"));
    }

    #[test]
    fn format_str() {
        let mut buf = [0u8; 32];
        let wrote = "hello".encode_to(&mut buf);
        let result = format_one(<&str as Encode>::schema(), &buf[..wrote]);
        assert_eq!(result, "hello");
    }

    #[test]
    fn format_bool() {
        let result = format_one(<bool as Encode>::schema(), &[1]);
        assert_eq!(result, "true");
        let result = format_one(<bool as Encode>::schema(), &[0]);
        assert_eq!(result, "false");
    }

    #[test]
    fn format_char() {
        let mut buf = [0u8; 4];
        'X'.encode_to(&mut buf);
        let result = format_one(<char as Encode>::schema(), &buf);
        assert_eq!(result, "X");
    }

    #[test]
    fn format_unit() {
        let result = format_one(<() as Encode>::schema(), &[]);
        assert_eq!(result, "()");
    }

    #[test]
    fn format_large_i32() {
        let val: i32 = 0x7FFF_FFFF;
        let mut buf = [0u8; 4];
        val.encode_to(&mut buf);
        let result = format_one(<i32 as Encode>::schema(), &buf);
        assert_eq!(result, "2147483647");
    }

    #[test]
    fn format_large_u32() {
        let val: u32 = 0xFFFF_FFFF;
        let mut buf = [0u8; 4];
        val.encode_to(&mut buf);
        let result = format_one(<u32 as Encode>::schema(), &buf);
        assert_eq!(result, "4294967295");
    }

    #[test]
    fn format_large_i64() {
        let val: i64 = 0x7FFF_FFFF_FFFF_FFFF;
        let mut buf = [0u8; 8];
        val.encode_to(&mut buf);
        let result = format_one(<i64 as Encode>::schema(), &buf);
        assert_eq!(result, "9223372036854775807");
    }

    // ── format_payload compounds ───────────────────────────────────────

    #[test]
    fn format_seq_i32() {
        let mut buf = [0u8; 64];
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
        let schema = [
            OP_STRUCT | 2,
            0u8,
            0u8,
            0u8,
            op_signed_int(3),
            2u8,
            0u8,
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
            1u8,
            0u8,
            0u8,
            0u8,
            OP_UNIT,
            5u8,
            0u8,
            op_signed_int(3),
        ];
        let st: &[u8] = b"None\0Some\0";
        let payload = [0u8];
        let mut s = String::new();
        format_payload(&schema, &payload, &[st], &[], &mut s);
        assert_eq!(s, "None");

        let mut payload = [0u8; 8];
        payload[0] = 1;
        payload[1..5].copy_from_slice(&42i32.to_ne_bytes());
        let mut s = String::new();
        format_payload(&schema, &payload, &[st], &[], &mut s);
        assert_eq!(s, "Some(42)");
    }

    #[test]
    fn format_struct_u16() {
        let mut buf = [0u8; 64];
        let mut pos = 0;
        pos += 100i32.encode_to(&mut buf[pos..]);
        pos += "abc".encode_to(&mut buf[pos..]);
        let st: &[&[u8]] = &[b"field1\0field2\0"];
        let schema = [
            OP_STRUCT,
            2u8,
            0u8,
            0u8,
            0u8,
            0u8,
            op_signed_int(3),
            7u8,
            0u8,
            op_str(2),
        ];
        let mut s = String::new();
        format_payload(&schema, &buf[..pos], st, &[], &mut s);
        assert_eq!(s, "{field1: 100, field2: abc}");
    }

    #[test]
    fn format_enum_u16() {
        let schema = [
            OP_ENUM,
            2u8,
            0u8,
            1u8,
            0u8,
            0u8,
            0u8,
            OP_UNIT,
            2u8,
            0u8,
            op_signed_int(3),
        ];
        let st: &[&[u8]] = &[b"A\0B\0"];
        let payload = [0u8];
        let mut s = String::new();
        format_payload(&schema, &payload, st, &[], &mut s);
        assert_eq!(s, "A");

        let mut payload = [0u8; 8];
        payload[0] = 1;
        payload[1..5].copy_from_slice(&42i32.to_ne_bytes());
        let mut s = String::new();
        format_payload(&schema, &payload, st, &[], &mut s);
        assert_eq!(s, "B(42)");
    }

    // ── format_payload edge cases ──────────────────────────────────────

    #[test]
    fn format_empty_schema() {
        let result = format_one(&[], &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn format_truncated_payload() {
        let schema = [op_signed_int(3)];
        let result = format_one(&schema, &[0u8; 1]);
        assert!(result.is_empty());

        let schema = [op_str(2)];
        let result = format_one(&schema, &[0u8; 1]);
        assert!(result.is_empty());
    }

    #[test]
    fn format_zero_count_seq() {
        let schema = [OP_SEQ | 2, op_signed_int(3)];
        let payload = [0u8, 0u8];
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
}
