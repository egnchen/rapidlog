Final Plan: Custom Serialization Format (Phase 3)
Rationale
rkyv is currently dead weight — the #[derive(Archive)] on LogMessage generates ArchivedLogMessage as a type definition only. Actual encode/decode is manual ptr::read_unaligned. The arg tag system (TAG_INT, TAG_FLOAT, TAG_STR, TAG_FORMATTED) is flat and can't represent tuples, structs, enums, or containers without allocations. We replace both with a self-describing recursive schema bytecode embedded in Metadata.
Schema bytecode (docs/schema.md and src/arg.rs)
One byte per opcode. Upper 4 bits = type category, lower 4 bits = variant/size.
── Primitives ──
0x0k  Unit          0 bytes
0x1k  SignedInt     k: 1=i8,2=i16,3=i32,4=i64,5=i128   → k bytes LE
0x2k  UnsignedInt   k: 1=u8,2=u16,3=u32,4=u64,5=u128   → k bytes LE
0x3k  Float         k: 1=f8,2=f16,3=f32,4=f64,5=f128   → k bytes LE
0x4k  Str           k: 1=u8len,2=u16len,3=u32len,4=u64len → k-byte LE len + UTF-8
0x50  Char          4 bytes UTF-32 LE
0x60  Bool          1 byte (0=false, 1=true)
── Compounds (recursive sub-schemas follow, boundaries found by parsing) ──
0x8k  SEQ(k)        [+ elem_schema…]       payload: k-byte LE count + elem₀·elem₁…
0x90  TUPLE(n=u16)  [+ n:u16 LE] ([+ elem_schema…])×n
0x9n  TUPLE(n=inline 1..15)  ([+ elem_schema…])×n
0xA0  STRUCT(n=u16) [+ n:u16 LE] [+ st_idx:u8] ([+ st_off:u16 LE] [+ field_schema…])×n
0xAn  STRUCT(n=inline 1..15) [+ st_idx:u8] ([+ st_off:u16 LE] [+ field_schema…])×n
0xB0  ENUM(n=u16)   [+ n:u16 LE] [+ vdx_k:u8] [+ st_idx:u8] ([+ st_off:u16 LE] [+ var_schema…])×n
0xBn  ENUM(n=inline 1..15) [+ vdx_k:u8] [+ st_idx:u8] ([+ st_off:u16 LE] [+ var_schema…])×n
── Extended ──
0xF0  USER_DEFINED  [+ idx:u8] → Metadata.user_formatters[idx]
String tables: per-type &'static [u8] with null-terminated C strings. Schema references a string table by st_idx:u8 (index into Metadata.string_tables[]) and a field name by st_off:u16 LE (byte offset within that table, reads until \0). MAP removed (represented as SEQ(TUPLE(2,K,V))). OPTION removed (represented as ENUM(2).vdx_k=1, [None→Unit, Some→T]). PREFORMATTED removed (represented as Str with formatted payload).
Metadata changes (src/metadata.rs)
pub struct Metadata {
    pub level: LogLevel,
    pub format_str: &'static str,
    pub file: &'static str,
    pub line: u32,
    pub module_path: &'static str,
    pub arg_schemas: &'static [&'static [u8]],    // one schema per fmt arg
    pub string_tables: &'static [&'static [u8]],  // per-type null-term'd name tables
    pub user_formatters: &'static [UserFormatter],
}
pub struct UserFormatter {
    pub format: fn(payload: &[u8], f: &mut fmt::Formatter) -> (usize, fmt::Result),
}
Encode trait (src/arg.rs)
pub trait Encode {
    const SCHEMA: &'static [u8];
    fn encode_to(&self, buf: &mut [u8]) -> usize;
    fn max_encoded_size(&self) -> usize;
}
pub trait HasStringTable {
    const STRING_TABLE: &'static [u8];
}
Native impls: i8–i128, u8–u128, f32/f64, bool, char, unit, &str, String. Fallback wrappers DebugArg<T: Debug> / DisplayArg<T: Display> encode as Str(u16) — format-in-place on the hot path then write to ring buffer.
Header (src/message.rs)
[timestamp_ns:u64 LE][metadata_ptr:u64 LE][logger_ptr:u64 LE][_reserved:u64 LE]
32 bytes. _reserved = 0 for now. No args_len — queue header already carries total payload size. decode() uses ptr::read_unaligned × 4. Remove rkyv dependency.
Hot path (src/macros.rs)
const _META: &Metadata = &Metadata::new(
    level, fmt, file!(), line!(), module_path!(),
    &[$( <$arg as Encode>::SCHEMA ),*],
    &[/* string_tables for struct/enum args */],
    &[],
);
ThreadContext::push_encoded(HEADER_SIZE + _total_payload, |__buf| {
    // Write independent header fields, then timestamp last
    // Then payloads only — no tags, no count in buffer
}).ok();
Backend (src/backend.rs)
let header = ArchivedHeader::decode(raw)?;
let payload = &raw[HEADER_SIZE..];  // no heap copy
let metadata = unsafe { &*(header.metadata_ptr as *const Metadata) };
// Walk arg_schemas + payload in lockstep via format_payload()
format_payload(schema, payload, string_tables, user_formatters, f) — recursive zero-copy, zero-alloc formatter. Reads counts/lengths from payload bytes. Writes directly to &mut fmt::Formatter. Returns (bytes_consumed, fmt::Result).
Derive macro (rapidlog_derive/)
Proc-macro crate generating Encode + HasStringTable impls for structs and enums. Generates schema bytecode with field/variant names in a null-terminated string table, and encode_to() walking fields sequentially.
Benchmark (benches/logging.rs)
Move vec!["alpha".to_string(), ...] outside the loop, use DebugArg(black_box(&v)).
Dependency cleanup (Cargo.toml)
Remove: rkyv, rkyv_derive, bytecheck, time. Add: rapidlog_derive.
Documentation (docs/schema.md)
A markdown file in the project root documenting the complete schema bytecode format in natural language, with examples for each opcode.
---
Execution order
Step	Task
1	Define schema opcode constants
2	Define Encode + HasStringTable traits
3	Implement Encode for all primitives (i8–i128, u8–u128, f32,f64, bool, char, unit, &str, String)
4	Implement DebugArg<T> / DisplayArg<T> wrappers
5	Extend Metadata with arg_schemas, string_tables, user_formatters; update const fn new()
6	Replace ArchivedLogMessage with ArchivedHeader (32B, reserved 8B), update decode()
7	Rewrite log_impl! macro (schemas→Metadata, header+payload writes, timestamp last)
8	Implement recursive format_payload() parser (zero-alloc)
9	Update backend: ArchivedHeader::decode + format_payload(), remove decode_args/format_with_args
10	Create rapidlog_derive/ proc-macro crate
11	Optimize benchmark (move Vec alloc outside loop)
12	Remove rkyv, rkyv_derive, bytecheck, time from Cargo.toml; add rapidlog_derive
13	Write docs/schema.md — complete schema bytecode specification
14	Update all tests; cargo check → cargo test → cargo clippy → cargo fmt
---
