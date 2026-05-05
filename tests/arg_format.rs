use rapidlog::LogLevel;
use rapidlog::arg::{Encode, decode_args, format_body, op_float, op_signed_int};

fn schema_bytes_i32_f64() -> &'static [u8] {
    Box::leak(Box::new([2u8, op_signed_int(3), op_float(4)]))
}

fn schema_bytes_one_i32() -> &'static [u8] {
    Box::leak(Box::new([1u8, op_signed_int(3)]))
}

fn empty_schemas() -> &'static [u8] {
    &[]
}

fn schema_i32_f64() -> &'static [u8] {
    Box::leak(Box::new([2u8, op_signed_int(3), op_float(4)]))
}

// ── format_body tests ──────────────────────────────────────────────────────

#[test]
fn format_body_simple() {
    let metadata = rapidlog::metadata::Metadata::new(
        LogLevel::Info,
        "x: {}, y: {}",
        "test.rs",
        1,
        "test",
        schema_bytes_i32_f64,
        rapidlog::metadata::empty_string_tables_provider,
        rapidlog::metadata::empty_user_formatters_provider,
    );
    let mut payload = vec![0u8; 4 + 8];
    42i32.encode_to(&mut payload[..]);
    3.14f64.encode_to(&mut payload[4..]);
    let result = format_body(&metadata, &payload);
    assert!(result.contains("x: 42"), "got: {result}");
    assert!(result.contains("y: 3.14"), "got: {result}");
}

#[test]
fn format_body_no_args() {
    let metadata = rapidlog::metadata::Metadata::new(
        LogLevel::Info,
        "no args",
        "test.rs",
        1,
        "test",
        empty_schemas,
        rapidlog::metadata::empty_string_tables_provider,
        rapidlog::metadata::empty_user_formatters_provider,
    );
    let result = format_body(&metadata, &[]);
    assert_eq!(result, "no args");
}

#[test]
fn format_body_fewer_args_than_placeholders() {
    let metadata = rapidlog::metadata::Metadata::new(
        LogLevel::Info,
        "a: {}, b: {}",
        "test.rs",
        1,
        "test",
        schema_bytes_one_i32,
        rapidlog::metadata::empty_string_tables_provider,
        rapidlog::metadata::empty_user_formatters_provider,
    );
    let mut payload = vec![0u8; 4];
    42i32.encode_to(&mut payload[..]);
    let result = format_body(&metadata, &payload);
    assert!(result.contains("a: 42"));
}

#[test]
fn format_body_with_debug_spec() {
    let metadata = rapidlog::metadata::Metadata::new(
        LogLevel::Info,
        "val: {:?}",
        "test.rs",
        1,
        "test",
        schema_bytes_one_i32,
        rapidlog::metadata::empty_string_tables_provider,
        rapidlog::metadata::empty_user_formatters_provider,
    );
    let mut payload = vec![0u8; 4];
    99i32.encode_to(&mut payload[..]);
    let result = format_body(&metadata, &payload);
    assert!(result.contains("val: 99"), "got: {result}");
}

#[test]
fn format_body_extra_placeholders() {
    let metadata = rapidlog::metadata::Metadata::new(
        LogLevel::Info,
        "a: {}, b: {}",
        "test.rs",
        1,
        "test",
        schema_bytes_one_i32,
        rapidlog::metadata::empty_string_tables_provider,
        rapidlog::metadata::empty_user_formatters_provider,
    );
    let mut payload = vec![0u8; 4];
    5i32.encode_to(&mut payload[..]);
    let result = format_body(&metadata, &payload);
    assert!(result.contains("a: 5"));
    assert!(!result.contains("b: 5"));
}

#[test]
fn format_body_empty_payload() {
    let metadata = rapidlog::metadata::Metadata::new(
        LogLevel::Info,
        "hello",
        "test.rs",
        1,
        "test",
        empty_schemas,
        rapidlog::metadata::empty_string_tables_provider,
        rapidlog::metadata::empty_user_formatters_provider,
    );
    let result = format_body(&metadata, &[]);
    assert_eq!(result, "hello");
}

// ── decode_args tests ───────────────────────────────────────────────────────

#[test]
fn decode_args_two_ints() {
    let metadata = rapidlog::metadata::Metadata::new(
        LogLevel::Info,
        "",
        "",
        0,
        "",
        schema_i32_f64,
        rapidlog::metadata::empty_string_tables_provider,
        rapidlog::metadata::empty_user_formatters_provider,
    );
    let mut buf = [0u8; 16];
    let mut pos = 0;
    pos += 42i32.encode_to(&mut buf[pos..]);
    3.14f64.encode_to(&mut buf[pos..]);
    let decoded = decode_args(&metadata, &buf);
    assert_eq!(decoded.len(), 2);
    assert_eq!(decoded[0], "42");
    assert!(decoded[1].contains("3.14"));
}

#[test]
fn decode_args_empty() {
    let metadata = rapidlog::metadata::Metadata::new(
        LogLevel::Info,
        "",
        "",
        0,
        "",
        empty_schemas,
        rapidlog::metadata::empty_string_tables_provider,
        rapidlog::metadata::empty_user_formatters_provider,
    );
    assert!(decode_args(&metadata, &[]).is_empty());
}

#[test]
fn decode_args_zero_arg_count() {
    let metadata = rapidlog::metadata::Metadata::new(
        LogLevel::Info,
        "",
        "",
        0,
        "",
        empty_schemas,
        rapidlog::metadata::empty_string_tables_provider,
        rapidlog::metadata::empty_user_formatters_provider,
    );
    assert!(decode_args(&metadata, &[]).is_empty());
}

#[test]
fn decode_args_truncated_payload() {
    let metadata = rapidlog::metadata::Metadata::new(
        LogLevel::Info,
        "",
        "",
        0,
        "",
        schema_bytes_one_i32,
        rapidlog::metadata::empty_string_tables_provider,
        rapidlog::metadata::empty_user_formatters_provider,
    );
    let decoded = decode_args(&metadata, &[0u8]);
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0], "");
}

// ── st_idx positional alignment test ────────────────────────────────────────

static ST_IDX_TABLES: [&[u8]; 2] = [b"None\0Some\0", b"Absent\0Present\0"];

fn st_idx_tables_provider() -> &'static [&'static [u8]] {
    &ST_IDX_TABLES
}

fn st_idx_schema_provider() -> &'static [u8] {
    Box::leak({
        let v: Vec<u8> = vec![
            2, // arg count
            0xB2,
            1, // OP_ENUM|2, vdx_k=1
            0, // st_idx=0
            0,
            0,    // st_off=0 → "None"
            0x00, // OP_UNIT
            5,
            0,                // st_off=5 → "Some"
            op_signed_int(3), // i32
            // Second arg
            0xB2,
            1,
            0,
            0,
            0,
            0x00,
            5,
            0,
            op_signed_int(3),
        ];
        v.into_boxed_slice()
    })
}

/// Verifies BUG #1 fix: each arg's schema references its own string table entry.
#[test]
fn st_idx_positional_alignment() {
    let metadata = rapidlog::metadata::Metadata::new(
        LogLevel::Info,
        "a: {}, b: {}",
        "test.rs",
        1,
        "test",
        st_idx_schema_provider,
        st_idx_tables_provider,
        rapidlog::metadata::empty_user_formatters_provider,
    );

    // Arg 0: Some(42), Arg 1: None
    let mut payload = vec![0u8; 1 + 4 + 1];
    payload[0] = 1;
    payload[1..5].copy_from_slice(&42i32.to_ne_bytes());
    payload[5] = 0;

    let result = format_body(&metadata, &payload);
    assert!(result.contains("a: Some(42)"), "got: {result}");
    assert!(result.contains("b: Absent"), "got: {result}");
}
