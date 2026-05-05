// Re-exports for external use
pub(crate) mod compound;
pub(crate) mod container;
pub(crate) mod pointer;
pub(crate) mod primitives;
pub(crate) mod schema;
pub(crate) mod traits;

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
pub(crate) fn read_unsigned_ne(buf: &[u8], k: u8) -> u64 {
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
pub(crate) fn read_signed_ne(buf: &[u8], k: u8) -> i64 {
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

// Re-export public items from submodules
pub use primitives::{DebugArg, DisplayArg};
pub use schema::{decode_args, format_body, format_payload, measure_schema};
pub use traits::{Encode, HasStringTable, SchemaOf, UserFormatter};
pub use traits::{schema_len, schema_of, string_table_of};
