use super::traits::Encode;

// ── Box<T> Encode ───────────────────────────────────────────────────────────

impl<T: std::fmt::Display> Encode for Box<T> {
    fn schema() -> &'static [u8] {
        <&str as Encode>::schema()
    }

    fn encode_to(&self, buf: &mut [u8]) -> usize {
        self.to_string().as_str().encode_to(buf)
    }

    fn encoded_size(&self) -> usize {
        512
    }
}

// ── Arc<T> Encode ───────────────────────────────────────────────────────────

impl<T: std::fmt::Display> Encode for std::sync::Arc<T> {
    fn schema() -> &'static [u8] {
        <&str as Encode>::schema()
    }

    fn encode_to(&self, buf: &mut [u8]) -> usize {
        self.to_string().as_str().encode_to(buf)
    }

    fn encoded_size(&self) -> usize {
        512
    }
}

// ── Rc<T> Encode ────────────────────────────────────────────────────────────

impl<T: std::fmt::Display> Encode for std::rc::Rc<T> {
    fn schema() -> &'static [u8] {
        <&str as Encode>::schema()
    }

    fn encode_to(&self, buf: &mut [u8]) -> usize {
        self.to_string().as_str().encode_to(buf)
    }

    fn encoded_size(&self) -> usize {
        512
    }
}
