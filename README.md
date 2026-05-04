# rapidlog

A high-performance asynchronous logging library for Rust, ported from the
[Quill C++ logging library](https://github.com/odygrd/quill).

Caller threads push encoded messages into per-thread lock-free SPSC queues.
A single backend worker thread pops, sorts by timestamp, formats, and
dispatches to output sinks.

## Quick Start

```rust
use rapidlog::{Backend, BackendOptions, ConsoleSink, Frontend, LogLevel};

fn main() {
    // Create a logger with a console sink
    let logger = Frontend::create_or_get_logger(
        "app",
        vec![std::sync::Arc::new(ConsoleSink::new())],
    );
    logger.set_log_level(LogLevel::Debug);

    // Start the backend worker
    let backend = Backend::start(BackendOptions::default());

    // Log from any thread
    Frontend::preallocate();
    rapidlog::log_info!(logger, "Hello {}", "world");
    rapidlog::log_debug!(logger, "Value: {}", 42);
    rapidlog::log_error!(logger, "Something went wrong");

    // Stop and flush
    backend.stop();
    let mut backend = backend;
    backend.join();
}
```

## Architecture

```
┌── Caller Thread ───────────────────────────────────┐
│ log_info!(logger, "x={}", 42)                      │
│   → encode args → push (ts, meta*, logger*, data)  │
│     into thread-local SPSC queue                    │
└──────────────────┬─────────────────────────────────┘
                   │ lock-free SPSC (rtrb::RingBuffer<u8>)
┌──────────────────▼─────────────────────────────────┐
│ Backend Worker Thread (single)                     │
│   → pop all thread queues                          │
│   → sort batch by timestamp                        │
│   → evaluate runtime filters                       │
│   → format with std::fmt                           │
│   → dispatch to all sinks on the logger            │
└────────────────────────────────────────────────────┘
```

## Log Levels

| Level | Macro | Numeric |
|-------|-------|---------|
| TraceL3 | `log_trace_l3!` | 0 |
| TraceL2 | `log_trace_l2!` | 1 |
| TraceL1 | `log_trace_l1!` | 2 |
| Debug | `log_debug!` | 3 |
| Info | `log_info!` | 4 |
| Warning | `log_warning!` | 5 |
| Error | `log_error!` | 6 |
| Critical | `log_critical!` | 7 |

## Features

### Compile-time Level Filtering

Use Cargo features to strip log calls at compile time:

```toml
[dependencies]
rapidlog = { features = ["max_level_info"] }
```

With `max_level_info` enabled, `log_debug!`, `log_trace_l1!`, etc. compile to
nothing — zero cost, arguments never evaluated.

| Feature | Disables |
|---------|----------|
| `max_level_debug` | TraceL3, TraceL2, TraceL1 |
| `max_level_info` | Debug, TraceL3, TraceL2, TraceL1 |
| `max_level_warning` | Info, Debug, TraceL3, TraceL2, TraceL1 |
| `max_level_error` | Warning, Info, Debug, and all trace |

`max_level_trace` (default) disables nothing. `log_error!` and `log_critical!`
are never disabled.

### TSC Timestamps (x86_64 only)

```toml
rapidlog = { features = ["tsc_clock"] }
```

Uses `RDTSC` instruction for timestamps (~5 ns) instead of `SystemTime::now()`
(~20 ns via vDSO). TSC ticks are stored raw in the queue and converted to
wall-clock nanoseconds on the backend thread. Falls back to system clock on
non-x86_64 targets.

### Runtime Filters

```rust
use rapidlog::{LevelFilter, LogLevel};

logger.add_filter(std::sync::Arc::new(LevelFilter::new(LogLevel::Warning)));
// Only Warning, Error, Critical messages are delivered.

logger.clear_filters();
```

Filters are AND-combined. Implement `Filter` trait for custom logic:

```rust
use rapidlog::{DecodedArg, Filter, Metadata};

struct RateLimitFilter { /* ... */ }

impl Filter for RateLimitFilter {
    fn accept(&self, metadata: &Metadata, args: &[DecodedArg]) -> bool {
        // Custom logic here
        true
    }
}
```

## Argument Types

| Rust Type | Encoded As |
|-----------|-----------|
| `i32`, `i64`, `u32`, `u64`, `usize`, `bool` | 8-byte integer |
| `f32`, `f64` | 8-byte float |
| `&str`, `String` | 2-byte length + UTF-8 bytes |
| Custom `Display`/`Debug` | `DisplayArg<T>`, `DebugArg<T>` wrappers |

```rust
use rapidlog::arg::DebugArg;

let vec = vec![1, 2, 3];
rapidlog::log_info!(logger, "data: {:?}", DebugArg(&vec));
```

## Sinks

| Sink | Description |
|------|-------------|
| `ConsoleSink` | ANSI-colored stdout via `anstream` |
| `NullSink` | No-op sink for benchmarks and tests |
| `FileSink` | Buffered file output with append/truncate modes |
| `RotatingFileSink` | File rotation by size (N bytes) or time (hourly/daily) |

Additional sinks planned: `JsonFileSink`, `JsonConsoleSink`.

### FileSink

```rust
use rapidlog::{FileMode, FileSink};

let sink = FileSink::new("app.log", FileMode::Append).unwrap();
let logger = Frontend::create_or_get_logger("file", vec![Arc::new(sink)]);
// Messages are written to app.log, one per line.
```

- `FileMode::Append` — create if missing, append to existing
- `FileMode::Truncate` — create or overwrite

### RotatingFileSink

```rust
use rapidlog::{RotatingFileSink, RotationPolicy};

// Rotate after 10 MB
let policy = RotationPolicy::SizeBased { max_bytes: 10 * 1024 * 1024 };
let sink = RotatingFileSink::new("app.log", policy).unwrap();

// Rotate every hour
use rapidlog::TimeInterval;
let policy = RotationPolicy::TimeBased { interval: TimeInterval::Hourly };
let sink = RotatingFileSink::new("app.log", policy).unwrap();
```

Rotated files are named `app.log.1`, `app.log.2`, etc. Compression is not yet
supported.

## Pattern Formatter

Customize log message format via `BackendOptions`:

```rust
use rapidlog::{Backend, BackendOptions, PatternFormatter};

let pattern = PatternFormatter::new("%Y-%m-%d %H:%M:%S.%f [%l] %F:%L — %v");
let options = BackendOptions {
    pattern_formatter: Some(pattern),
    ..Default::default()
};
let backend = Backend::start(options);
```

When no formatter is provided, the default format is:
`"[{timestamp_secs}.{nanos}] [{level}] {file}:{line} {message}"`

| Specifier | Output |
|-----------|--------|
| `%Y` | Year (4 digits) |
| `%m` | Month (01–12) |
| `%d` | Day (01–31) |
| `%H` | Hour (00–23) |
| `%M` | Minute (00–59) |
| `%S` | Second (00–59) |
| `%s` | Whole seconds (Unix timestamp) |
| `%f` | Fractional seconds (9-digit nanoseconds) |
| `%l` | Log level (e.g. `Info`, `Error`) |
| `%F` | Source file path |
| `%L` | Source line number |
| `%v` | Formatted message body |

Unknown specifiers (e.g. `%x`) are rendered literally.

## Queue Modes

Control overflow behavior per thread via `Frontend::set_queue_mode()`:

```rust
use rapidlog::QueueMode;

// Never drop messages — grow queue as needed (default starting at 128 KiB)
Frontend::set_queue_mode(QueueMode::UnboundedBlocking);

// Drop messages when the fixed-size queue is full (legacy behavior)
Frontend::set_queue_mode(QueueMode::BoundedDropping);
```

| Mode | Behavior |
|------|----------|
| `UnboundedBlocking` | Grows by doubling capacity (up to 2 GiB) when full; never drops |
| `BoundedDropping` | Fixed 128 KiB ring buffer; silently drops messages when full |

Call `set_queue_mode` once per thread before logging. The mode applies to all
log calls on that thread, regardless of which logger is used.

## Benchmarks

Latency per log call (hot path, release build, TSC clock enabled):

| Benchmark | Time |
|-----------|------|
| 1 integer | **~4.0 ns** |
| 2 floats | **~4.6 ns** |
| 3 strings | **~6.0 ns** |
| 1 Vec\<String\> (via DebugArg) | **~33 ns** |

Measured on x86_64 Linux with criterion. Includes timestamp acquisition,
argument encoding, and SPSC queue push. Backend formatting and dispatch
are not included.

## Minimum Supported Rust Version

Rust 1.95+ (edition 2024)
