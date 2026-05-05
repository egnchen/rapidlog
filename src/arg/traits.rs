use std::fmt;

// ── Encode trait ───────────────────────────────────────────────────────────

pub trait Encode {
    fn schema() -> &'static [u8];
    fn encode_to(&self, buf: &mut [u8]) -> usize;
    fn encoded_size(&self) -> usize;
    fn string_table(&self) -> &'static [u8] {
        &[]
    }
}

/// Returns the schema for any `Encode` implementor.
/// Used by macros to get the schema from an expression (not a type).
#[inline]
pub fn schema_of<T: Encode + ?Sized>(_: &T) -> &'static [u8] {
    T::schema()
}

/// Returns the schema length for an `Encode` implementor.
/// Used by macros to compute total encoding size.
#[inline]
pub fn schema_len<T: Encode + ?Sized>(_: &T) -> usize {
    T::schema().len()
}

pub trait HasStringTable {
    const STRING_TABLE: &'static [u8];
}

/// Helper: calls `HasStringTable::STRING_TABLE` for types implementing it.
/// For types without `HasStringTable`, use `Encode::string_table` instead.
#[inline]
pub fn string_table_of<T: HasStringTable + ?Sized>(_: &T) -> &'static [u8] {
    <T as HasStringTable>::STRING_TABLE
}

// ── UserFormatter ──────────────────────────────────────────────────────────

pub struct UserFormatter {
    pub format: fn(payload: &[u8], f: &mut dyn fmt::Write) -> (usize, fmt::Result),
}

// ── SchemaOf trait (type-level witness to disambiguate super::Encode) ──────

/// Type witness trait: allows getting schema of a type without
/// potential for the compiler to resolve to the calling function's impl.
pub trait SchemaOf {
    fn schema_of() -> &'static [u8];
}

impl<T: Encode> SchemaOf for T {
    #[inline]
    fn schema_of() -> &'static [u8] {
        <T as Encode>::schema()
    }
}
