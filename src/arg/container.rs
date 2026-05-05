use super::traits::{Encode, SchemaOf};
use super::{OP_SEQ, OP_TUPLE};

// ── Vec<T> Encode ───────────────────────────────────────────────────────────

impl<T: Encode> Encode for Vec<T> {
    fn schema() -> &'static [u8] {
        Box::leak({
            let mut v = Vec::new();
            v.push(OP_SEQ | 2);
            v.extend_from_slice(<T as SchemaOf>::schema_of());
            v.into_boxed_slice()
        })
    }

    fn encode_to(&self, buf: &mut [u8]) -> usize {
        let count = self.len().min(u16::MAX as usize) as u16;
        buf[..2].copy_from_slice(&count.to_ne_bytes());
        let mut pos = 2;
        for item in self.iter().take(count as usize) {
            pos += item.encode_to(&mut buf[pos..]);
        }
        pos
    }

    fn encoded_size(&self) -> usize {
        let count = self.len().min(u16::MAX as usize);
        2 + self
            .iter()
            .take(count)
            .map(|e| e.encoded_size())
            .sum::<usize>()
    }
}

// ── HashMap<K,V> Encode ─────────────────────────────────────────────────────

impl<K: Encode + std::hash::Hash + Eq, V: Encode> Encode for std::collections::HashMap<K, V> {
    fn schema() -> &'static [u8] {
        Box::leak({
            let mut v = Vec::new();
            v.push(OP_SEQ | 3);
            v.push(OP_TUPLE | 2);
            v.extend_from_slice(<K as SchemaOf>::schema_of());
            v.extend_from_slice(<V as SchemaOf>::schema_of());
            v.into_boxed_slice()
        })
    }

    fn encode_to(&self, buf: &mut [u8]) -> usize {
        let len = (self.len() as u32).to_ne_bytes();
        buf[0] = len[0];
        buf[1] = len[1];
        buf[2] = len[2];
        buf[3] = len[3];
        let mut pos = 4;
        for (k, v) in self {
            pos += k.encode_to(&mut buf[pos..]);
            pos += v.encode_to(&mut buf[pos..]);
        }
        pos
    }

    fn encoded_size(&self) -> usize {
        if self.is_empty() {
            return 4;
        }
        let k = self.iter().next().unwrap().0.encoded_size();
        let v = self.iter().next().unwrap().1.encoded_size();
        4 + self.len().saturating_mul(k + v)
    }
}

// ── BTreeMap<K,V> Encode ────────────────────────────────────────────────────

impl<K: Encode + Ord, V: Encode> Encode for std::collections::BTreeMap<K, V> {
    fn schema() -> &'static [u8] {
        Box::leak({
            let mut v = Vec::new();
            v.push(OP_SEQ | 3);
            v.push(OP_TUPLE | 2);
            v.extend_from_slice(<K as SchemaOf>::schema_of());
            v.extend_from_slice(<V as SchemaOf>::schema_of());
            v.into_boxed_slice()
        })
    }

    fn encode_to(&self, buf: &mut [u8]) -> usize {
        let len = (self.len() as u32).to_ne_bytes();
        buf[0] = len[0];
        buf[1] = len[1];
        buf[2] = len[2];
        buf[3] = len[3];
        let mut pos = 4;
        for (k, v) in self {
            pos += k.encode_to(&mut buf[pos..]);
            pos += v.encode_to(&mut buf[pos..]);
        }
        pos
    }

    fn encoded_size(&self) -> usize {
        if self.is_empty() {
            return 4;
        }
        let k = self.iter().next().unwrap().0.encoded_size();
        let v = self.iter().next().unwrap().1.encoded_size();
        4 + self.len().saturating_mul(k + v)
    }
}
