use rapidlog::thread_context::ThreadContext;
use rapidlog::{Frontend, LogLevel, NullSink};
use std::sync::Arc;
use std::time::Instant;

fn main() {
    let logger = Frontend::create_or_get_logger("verify", vec![Arc::new(NullSink)]);
    logger.set_log_level(LogLevel::TraceL3);
    Frontend::preallocate();
    let _ = ThreadContext::poll_all_registered_queues();

    let iters = 20_000_000u64;
    let start = Instant::now();
    for _ in 0..iters {
        rapidlog::log_info!(logger, "x: {}", 42i32);
    }
    let mid = start.elapsed();
    let _ = ThreadContext::poll_all_registered_queues();
    let end = start.elapsed();

    eprintln!("{} iterations:", iters);
    eprintln!(
        "  Before drain: {:?} = {:.2} ns/iter",
        mid,
        mid.as_nanos() as f64 / iters as f64
    );
    eprintln!(
        "  After drain:  {:?} = {:.2} ns/iter",
        end,
        end.as_nanos() as f64 / iters as f64
    );
}
