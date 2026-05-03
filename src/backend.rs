pub struct BackendOptions {
    pub sleep_duration_ms: u64,
}

impl Default for BackendOptions {
    fn default() -> Self {
        Self {
            sleep_duration_ms: 1,
        }
    }
}

pub struct Backend;

impl Backend {
    pub fn start(_options: BackendOptions) {}
}
