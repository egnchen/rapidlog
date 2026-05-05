use super::traits::{Encode, SchemaOf};
use super::{OP_ENUM, OP_SEQ, OP_TUPLE, OP_UNIT};

// ── Tuple Encode impls ──────────────────────────────────────────────────────

macro_rules! impl_tuple_encode_one {
    ($count:literal ; $($T:ident),+) => {
        impl<$($T: Encode),+> Encode for ($($T,)+) {
            fn schema() -> &'static [u8] {
                Box::leak({
                    let mut v = Vec::new();
                    v.push(OP_TUPLE | $count);
                    $( v.extend_from_slice(<$T as SchemaOf>::schema_of()); )+
                    v.into_boxed_slice()
                })
            }

            fn encode_to(&self, buf: &mut [u8]) -> usize {
                #[allow(non_snake_case)]
                let ($(ref $T,)+) = *self;
                let mut pos = 0;
                $( pos += $T.encode_to(&mut buf[pos..]); )+
                pos
            }

            fn encoded_size(&self) -> usize {
                #[allow(non_snake_case)]
                let ($(ref $T,)+) = *self;
                0 $( + $T.encoded_size() )+
            }
        }
    };
}

macro_rules! impl_tuple_encode {
    ($($T:ident),+ $(,)?) => {
        impl_tuple_encode! {
            @walk
            [1 2 3 4 5 6 7 8 9 10 11 12]
            []
            $($T),+
        }
    };

    (@walk [$count:literal $($rest_counts:literal)*] [$($acc:ident,)*] $next:ident $(, $rest:ident)*) => {
        impl_tuple_encode_one!($count; $($acc,)* $next);

        impl_tuple_encode! {
            @walk
            [$($rest_counts)*]
            [$($acc,)* $next,]
            $($rest),*
        }
    };

    (@walk [$($counts:literal)*] [$($acc:ident,)*]) => {};
}

impl_tuple_encode!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);

// ── [T; N] Array Encode ─────────────────────────────────────────────────────

macro_rules! impl_array_encode {
    ($($n:literal),+) => {
        $(
            impl<T: Encode> Encode for [T; $n] {
                fn schema() -> &'static [u8] {
                    Box::leak({
                        let mut v = Vec::new();
                        v.push(OP_SEQ | 2);
                        v.extend_from_slice(<T as SchemaOf>::schema_of());
                        v.into_boxed_slice()
                    })
                }

                fn encode_to(&self, buf: &mut [u8]) -> usize {
                    let len = ($n as u16).to_ne_bytes();
                    buf[0] = len[0];
                    buf[1] = len[1];
                    let mut pos = 2;
                    for item in self {
                        pos += item.encode_to(&mut buf[pos..]);
                    }
                    pos
                }

                fn encoded_size(&self) -> usize {
                    #[allow(clippy::out_of_bounds_indexing)]
                    {
                        if $n == 0 {
                            2
                        } else {
                            2 + $n * self[0].encoded_size()
                        }
                    }
                }
            }
        )+
    };
}

impl_array_encode!(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 16, 32, 64, 128, 256);

// ── Option<T> Encode ────────────────────────────────────────────────────────

impl<T: Encode> Encode for Option<T> {
    fn schema() -> &'static [u8] {
        Box::leak({
            let mut v: Vec<u8> = vec![OP_ENUM | 2, 1, 0, 0, 0, OP_UNIT, 5, 0];
            v.extend_from_slice(<T as SchemaOf>::schema_of());
            v.into_boxed_slice()
        })
    }

    fn encode_to(&self, buf: &mut [u8]) -> usize {
        match self {
            None => {
                buf[0] = 0u8;
                1
            }
            Some(val) => {
                buf[0] = 1u8;
                1 + val.encode_to(&mut buf[1..])
            }
        }
    }

    fn encoded_size(&self) -> usize {
        match self {
            None => 1,
            Some(val) => 1 + val.encoded_size(),
        }
    }

    fn string_table(&self) -> &'static [u8] {
        b"None\0Some\0"
    }
}

// ── Result<T, E> Encode ─────────────────────────────────────────────────────

impl<T: Encode, E: Encode> Encode for Result<T, E> {
    fn schema() -> &'static [u8] {
        Box::leak({
            let mut v: Vec<u8> = vec![OP_ENUM | 2, 1, 0, 0, 0];
            v.extend_from_slice(<T as SchemaOf>::schema_of());
            v.extend_from_slice(&[3u8, 0u8]);
            v.extend_from_slice(<E as SchemaOf>::schema_of());
            v.into_boxed_slice()
        })
    }

    fn encode_to(&self, buf: &mut [u8]) -> usize {
        match self {
            Ok(val) => {
                buf[0] = 0u8;
                1 + val.encode_to(&mut buf[1..])
            }
            Err(val) => {
                buf[0] = 1u8;
                1 + val.encode_to(&mut buf[1..])
            }
        }
    }

    fn encoded_size(&self) -> usize {
        1 + match self {
            Ok(val) => val.encoded_size(),
            Err(val) => val.encoded_size(),
        }
    }

    fn string_table(&self) -> &'static [u8] {
        b"Ok\0Err\0"
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;

    fn fmt_tuple_payload(schema: &[u8], payload: &[u8]) -> String {
        let mut s = String::new();
        format_payload(schema, payload, &[], &[], &mut s);
        s
    }

    #[test]
    fn encode_decode_tuple_1_i32() {
        let val: (i32,) = (42,);
        let mut buf = [0u8; 64];
        let wrote = val.encode_to(&mut buf);
        let schema = <(i32,) as Encode>::schema();
        assert!(schema[0] & OP_TUPLE != 0);
        let result = fmt_tuple_payload(schema, &buf[..wrote]);
        assert_eq!(result, "(42)");
    }

    #[test]
    fn encode_decode_tuple_2_i32_f64() {
        let val: (i32, f64) = (42, 3.14);
        let mut buf = [0u8; 64];
        let wrote = val.encode_to(&mut buf);
        let schema = <(i32, f64) as Encode>::schema();
        assert!(schema[0] & OP_TUPLE != 0);
        let result = fmt_tuple_payload(schema, &buf[..wrote]);
        assert!(result.contains("42"));
        assert!(result.contains("3.14"));
    }

    #[test]
    fn encode_decode_tuple_3_mixed() {
        let val: (i32, &str, bool) = (1, "hello", true);
        let mut buf = [0u8; 128];
        let wrote = val.encode_to(&mut buf);
        let schema = <(i32, &str, bool) as Encode>::schema();
        let result = fmt_tuple_payload(schema, &buf[..wrote]);
        assert!(result.contains("1"));
        assert!(result.contains("hello"));
        assert!(result.contains("true"));
    }

    #[test]
    fn encode_decode_tuple_empty_0() {
        let mut buf = [0u8; 4];
        let wrote = ().encode_to(&mut buf);
        assert_eq!(wrote, 0);
        let result = fmt_tuple_payload(<() as Encode>::schema(), &[]);
        assert_eq!(result, "()");
    }

    #[test]
    fn encode_decode_tuple_12_max() {
        let val: (i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32) =
            (1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12);
        let mut buf = [0u8; 256];
        let wrote = val.encode_to(&mut buf);
        let schema =
            <(i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32) as Encode>::schema();
        assert!(measure_schema(schema) > 1);
        let result = fmt_tuple_payload(schema, &buf[..wrote]);
        for i in 1..=12 {
            assert!(result.contains(&i.to_string()), "missing {i} in {result:?}");
        }
    }

    #[test]
    fn tuple_schema_starts_with_op_tuple_n() {
        let schema = <(i32, f64) as Encode>::schema();
        assert!((schema[0] & CAT_MASK) == OP_TUPLE);
        assert!((schema[0] & SIZE_MASK) == 2);
    }

    #[test]
    fn tuple_encoded_size_equals_sum() {
        let val: (i32, f64) = (42, 3.14);
        let max = val.encoded_size();
        let actual = 42i32.encoded_size() + 3.14f64.encoded_size();
        assert_eq!(max, actual);
    }

    #[test]
    fn tuple_nested_1tuple() {
        let s1 = <(i32,) as Encode>::schema();
        assert_eq!(s1.len(), 2);

        let s2 = <((i32,), (i32,)) as Encode>::schema();
        assert_eq!(s2.len(), 5);
    }

    #[test]
    fn tuple_nested_tuple_2d() {
        let val: ((i32, i32), (i32, i32)) = ((1, 2), (3, 4));
        let mut buf = [0u8; 256];
        let wrote = val.encode_to(&mut buf);
        assert_eq!(wrote, 16);
        let schema = <((i32, i32), (i32, i32)) as Encode>::schema();
        assert_eq!(schema.len(), 7);
        let result = fmt_tuple_payload(schema, &buf[..wrote]);
        assert!(result.contains("((1, 2), (3, 4))"), "got: {result:?}");
    }

    #[test]
    fn tuple_measure_schema_correct() {
        let schema = <(i32, f64) as Encode>::schema();
        let measured = measure_schema(schema);
        assert_eq!(measured, schema.len());
    }
}
