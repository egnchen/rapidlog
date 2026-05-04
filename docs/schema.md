# Rapidlog Schema Bytecode

Rapidlog uses a self-describing recursive schema bytecode to encode log argument types. Each log message stores its argument schemas inline in the payload, enabling the backend to decode and format values without runtime type information or heap allocations.

## Opcodes

Each opcode is one byte: the upper 4 bits define the type category, and the lower 4 bits define a variant or size parameter.

### Primitives

| Opcode  | Mnemonic    | Payload size | Description                           |
|---------|-------------|-------------|---------------------------------------|
| `0x0k`  | Unit        | 0           | Unit type `()` — no payload           |
| `0x1k`  | SignedInt   | k bytes     | Signed integer, LE. k: 1=i8,2=i16,3=i32,4=i64,5=i128 |
| `0x2k`  | UnsignedInt | k bytes     | Unsigned integer, LE. k: 1=u8,2=u16,3=u32,4=u64,5=u128 |
| `0x3k`  | Float       | k bytes     | IEEE 754 float, LE. k: 3=f32,4=f64   |
| `0x4k`  | Str         | k + len     | k-byte LE length prefix + UTF-8 bytes. k: 1=u8len,2=u16len,3=u32len,4=u64len |
| `0x50`  | Char        | 4           | UTF-32 code point, LE                |
| `0x60`  | Bool        | 1           | 0 = false, 1 = true                  |

### Compounds

Compound opcodes contain recursive sub-schemas immediately following the opcode.

| Opcode  | Mnemonic    | Schema format                                                                 |
|---------|-------------|-------------------------------------------------------------------------------|
| `0x8k`  | SEQ(k)      | + elem_schema. Payload: k-byte LE count + repeated elem payloads              |
| `0x90`  | TUPLE(n)    | + n:u16 LE + n * elem_schema. n up to 65535                                   |
| `0x9n`  | TUPLE(n)    | + n * elem_schema. n inline in lower 4 bits (1–15)                            |
| `0xA0`  | STRUCT(n)   | + n:u16 LE + st_idx:u8 + (st_off:u16 LE + field_schema) × n                  |
| `0xAn`  | STRUCT(n)   | + st_idx:u8 + (st_off:u16 LE + field_schema) × n. n inline (1–15)            |
| `0xB0`  | ENUM(n)     | + n:u16 LE + vdx_k:u8 + st_idx:u8 + (st_off:u16 LE + var_schema) × n         |
| `0xBn`  | ENUM(n)     | + vdx_k:u8 + st_idx:u8 + (st_off:u16 LE + var_schema) × n. n inline (1–15)   |

### Extended

| Opcode  | Mnemonic     | Schema format                  |
|---------|-------------|--------------------------------|
| `0xF0`  | USER_DEFINED | + idx:u8. Calls Metadata.user_formatters[idx] |

## String Tables

Compound types (STRUCT, ENUM) reference field/variant names via string tables. Each type that implements `HasStringTable` provides a `&'static [u8]` — a concatenation of null-terminated field name strings.

The schema references a string table entry by `st_idx:u8` (index into `metadata.string_tables[]`) and `st_off:u16 LE` (byte offset within that table, reads until `\0`).

## Payload Format

A log message payload consists of:

```
[arg_count: u8] [schema₀] [schema₁] ... [schemaₙ] [payload₀] [payload₁] ... [payloadₙ]
```

Where each `schemaᵢ` is the complete schema bytecode for argument `i` (1+ bytes), and each `payloadᵢ` is the encoded value (variable length, determined by the schema).

## Examples

### Integer: `i32` = `42`

Schema: `[0x13]` (SignedInt, k=3 → 4 bytes)
Payload: `[0x2A, 0x00, 0x00, 0x00]`

### String: `"hello"`

Schema: `[0x42]` (Str, k=2 → u16 length prefix)
Payload: `[0x05, 0x00, 0x68, 0x65, 0x6C, 0x6C, 0x6F]` (len=5 + "hello")

### Boolean: `true`

Schema: `[0x60]`
Payload: `[0x01]`

### Sequence of 2 i32s: `[10, 20]`

Schema: `[0x82, 0x13]` (SEQ(k=2) + SignedInt(k=3))
Payload: `[0x02, 0x00, 0x0A, 0x00, 0x00, 0x00, 0x14, 0x00, 0x00, 0x00]`
(count=2 as u16 LE, then 10_i32 LE, then 20_i32 LE)

### Tuple of (i64, f64): `(5, 2.5)`

Schema: `[0x92, 0x14, 0x34]` (TUPLE(2) + SignedInt(4) + Float(4))
Payload: `[0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]` `[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x40]`
(5_i64 LE, then 2.5_f64 LE)

### Struct with 2 fields: `{x: 1i32, y: "xyz"}`

String table: `x\0y\0`

Schema:
```
[A2]                    STRUCT(2)
[00]                    st_idx = 0
[00 00]                 st_off = 0 → "x"
[13]                    field schema: SignedInt(3)
[02 00]                 st_off = 2 → "y"
[42]                    field schema: Str(2)
```

Payload:
```
[01 00 00 00]           1_i32 LE
[03 00]                 3_u16 LE (str len)
[78 79 7A]              "xyz"
```

### Enum with 2 variants: `Option<i32>` = Some(42)

String table: `None\0Some\0`

Schema:
```
[B2]                    ENUM(2)
[01]                    vdx_k = 1 (u8 discriminant)
[00]                    st_idx = 0
[00 00]                 st_off = 0 → "None"
[00]                    variant schema: Unit
[05 00]                 st_off = 5 → "Some"
[13]                    variant schema: SignedInt(3)
```

Payload for `Some(42)`:
```
[01]                    1_u8 (variant index 1)
[2A 00 00 00]           42_i32 LE
```

Payload for `None`:
```
[00]                    0_u8 (variant index 0)
```

## Hot Path (Macro)

The `log_impl!` macro:
1. Computes total size: 1 (count byte) + sum(schema sizes) + sum(max encoded sizes)
2. Writes count byte, then each arg's schema, then each arg's encoded payload

```rust
buf[HEADER_SIZE] = arg_count;
// ... write schemas ...
// ... write payloads ...
buf[0..8] = timestamp.to_le_bytes(); // written last
```

## Backend Decoding

The `format_body()` function:
1. Reads `arg_count` from `payload[0]`
2. Parses each schema (via `measure_schema()`) to know their boundaries
3. For each `{}` in the format string:
   - Takes the next schema
   - Calls `format_payload(schema, &payload[pos..], ...)` 
   - Advances `pos` by the bytes consumed
4. Returns the complete formatted string

`format_payload()` is a recursive zero-copy formatter that writes directly to `&mut dyn fmt::Write`, consuming no heap memory.
