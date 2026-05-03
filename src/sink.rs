pub trait Sink: Send + Sync {
    fn write(&self, formatted: &str);
    fn flush(&self);
}
