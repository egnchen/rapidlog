use std::collections::HashMap;
use std::hint::black_box;
use std::sync::Arc;

use criterion::{Criterion, criterion_group, criterion_main};
use rapidlog::arg::DebugArg;
use rapidlog::thread_context::ThreadContext;
use rapidlog::{Frontend, LogLevel, NullSink};

fn drain_queue() {
    let _ = ThreadContext::poll_all_registered_queues();
}

fn bench_one_integer(c: &mut Criterion) {
    let logger = Frontend::create_or_get_logger("bench_int", vec![Arc::new(NullSink)]);
    logger.set_log_level(LogLevel::TraceL3);

    c.bench_function("1 integer", |b| {
        b.iter_custom(|iters| {
            let l = logger.clone();
            Frontend::preallocate();
            drain_queue();
            let start = std::time::Instant::now();
            for _ in 0..iters {
                rapidlog::log_info!(l, "x: {}", black_box(42i32));
            }
            drain_queue();
            start.elapsed()
        });
    });
}

fn bench_two_floats(c: &mut Criterion) {
    let logger = Frontend::create_or_get_logger("bench_float", vec![Arc::new(NullSink)]);
    logger.set_log_level(LogLevel::TraceL3);

    c.bench_function("2 floats", |b| {
        b.iter_custom(|iters| {
            let l = logger.clone();
            Frontend::preallocate();
            drain_queue();
            let start = std::time::Instant::now();
            for _ in 0..iters {
                rapidlog::log_info!(l, "a: {}, b: {}", black_box(3.14f64), black_box(2.718f64));
            }
            drain_queue();
            start.elapsed()
        });
    });
}

fn bench_three_strings(c: &mut Criterion) {
    let logger = Frontend::create_or_get_logger("bench_str", vec![Arc::new(NullSink)]);
    logger.set_log_level(LogLevel::TraceL3);

    c.bench_function("3 strings", |b| {
        b.iter_custom(|iters| {
            let l = logger.clone();
            Frontend::preallocate();
            drain_queue();
            let start = std::time::Instant::now();
            for _ in 0..iters {
                rapidlog::log_info!(
                    l,
                    "a: {}, b: {}, c: {}",
                    black_box("first"),
                    black_box("second_and_longer"),
                    black_box("third"),
                );
            }
            drain_queue();
            start.elapsed()
        });
    });
}

fn bench_one_vec_string(c: &mut Criterion) {
    let logger = Frontend::create_or_get_logger("bench_vec", vec![Arc::new(NullSink)]);
    logger.set_log_level(LogLevel::TraceL3);

    let v: Vec<String> = vec![
        "alpha".to_string(),
        "beta_beta_beta".to_string(),
        "gamma".to_string(),
    ];

    c.bench_function("1 Vec<String>", |b| {
        b.iter_custom(|iters| {
            let l = logger.clone();
            Frontend::preallocate();
            drain_queue();
            let start = std::time::Instant::now();
            for _ in 0..iters {
                let dv = DebugArg(black_box(&v));
                rapidlog::log_info!(l, "vec: {:?}", dv);
            }
            drain_queue();
            start.elapsed()
        });
    });
}

fn bench_one_hashmap_string(c: &mut Criterion) {
    let logger = Frontend::create_or_get_logger("bench_hashmap", vec![Arc::new(NullSink)]);
    logger.set_log_level(LogLevel::TraceL3);

    let m: HashMap<String, String> = HashMap::from([
        ("key1".to_string(), "value1".to_string()),
        ("key2".to_string(), "value2_value2_value2".to_string()),
        ("key3".to_string(), "value3".to_string()),
    ]);

    c.bench_function("1 HashMap<String, String>", |b| {
        b.iter_custom(|iters| {
            let l = logger.clone();
            Frontend::preallocate();
            drain_queue();
            let start = std::time::Instant::now();
            for _ in 0..iters {
                let dm = DebugArg(black_box(&m));
                rapidlog::log_info!(l, "map: {:?}", dm);
            }
            drain_queue();
            start.elapsed()
        });
    });
}

criterion_group!(
    benches,
    bench_one_integer,
    bench_two_floats,
    bench_three_strings,
    bench_one_vec_string,
    bench_one_hashmap_string
);
criterion_main!(benches);
