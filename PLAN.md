# Rapidlog Implementation Plan

A high-performance asynchronous logging library for Rust, ported from the
[Quill C++ library][quill] (v11.1.0).

[quill]: https://github.com/odygrd/quill

## Architecture

```
┌── Caller Thread ────────────────────────────────┐
│ log_info!(logger, "x={} y={}", x, y)            │
│   → encode args (rkyv)                          │
│   → push (ts, meta*, logger*, data)             │
│     into thread-local SPSC queue                │
└────────────────┬────────────────────────────────┘
                 │ lock-free SPSC (rtrb::RingBuffer<u8>)
┌────────────────▼────────────────────────────────┐
│ Backend Worker Thread (single)                  │
│   → pop all thread queues                       │
│   → sort batch by timestamp                     │
│   → zero-copy access archived args (rkyv)       │
│   → format (std::fmt)                           │
│   → dispatch to all sinks on the logger         │
└─────────────────────────────────────────────────┘
```

### Key Design Decisions

| Concern | Decision | Rationale |
|---------|----------|-----------|
| SPSC queue | `rtrb::RingBuffer<u8>` with block chaining for unbounded growth | rigtorp algorithm (local index caching), zero deps, wait-free |
| Formatting | Rust `std::fmt` / `format_args!` | Native, fast, zero-dependency. Analog of Quill's `{fmt}` |
| Serialization (MVP) | `rkyv = "0.7"` zero-copy archive format | Fixed-offset encoding, no decode step — backend accesses fields at ~1 ns |
| Serialization (Phase 4) | Custom `DecodeFn` proc-macro (Quill-native pattern) | Replace rkyv with compile-time-generated per-callsite encode/decode pairs |
| Macros | Declarative (`macro_rules!`) → procedural (`#[proc_macro]`) | Fast iteration with declarative; optimal encoding with proc macros later |
| Queue modes | UnboundedBlocking only (MVP); rest in Phase 2 | 80/20: default mode covers most use cases |

---

## Phases

### Phase 1 — Core Foundation (MVP)

**Goal:** Async console logging working end-to-end with measurable latency.

#### 1.1 — Project scaffold & dependencies
- Crate: flat lib crate (`rapidlog`)
- Dependencies: `rtrb`, `rkyv = "0.7"`, `parking_lot`, `anstream`, `is-terminal`, `time`
- `lib.rs` re-exports public API

#### 1.2 — LogLevel (`src/level.rs`)
```rust
enum LogLevel {
    TraceL3, TraceL2, TraceL1,
    Debug, Info, Warning, Error, Critical,
}
```
`as_usize()` / `from_usize()` for compile-time filtering later.

#### 1.3 — Metadata (`src/metadata.rs`)
```rust
struct Metadata {
    level: LogLevel,
    format_str: &'static str,
    file: &'static str,
    line: u32,
    module: &'static str,
}
```
Created as `const` at each macro call site.

#### 1.4 — SPSC Queue (`src/queue.rs`)
- Wrap `rtrb::RingBuffer<u8>` for the hot path
- Implement length-prefixed message framing (header: `u32` length + payload)
- Unbounded mode: block-chaining layer (linked list of `rtrb::RingBuffer<u8>` nodes)
  - Start at 128 KiB, double each new block up to 2 GiB max
  - Producer creates new blocks, consumer frees drained ones
- `PushError::Full` handling: allocate new block (unbounded) or signal backpressure

#### 1.5 — Sink trait + ConsoleSink (`src/sink.rs`, `src/sinks/console.rs`)
```rust
trait Sink: Send + Sync {
    fn write(&self, formatted: &str);
    fn flush(&self);
}
```
`ConsoleSink`: ANSI coloring via `anstream`, TTY detection via `is-terminal`.

#### 1.6 — Logger (`src/logger.rs`)
```rust
struct Logger {
    name: String,
    level: AtomicU8,
    sinks: Vec<Arc<dyn Sink>>,
}
```
Global registry: `parking_lot::RwLock<HashMap<String, Arc<Logger>>>`.

#### 1.7 — Backend thread (`src/backend.rs`)
- `Backend::start(BackendOptions)` spawns single worker thread
- Worker loop:
  1. Poll all registered thread contexts for pending messages
  2. Accumulate batch, sort by timestamp
  3. For each message: zero-copy access archived args via rkyv, format with `std::fmt`
  4. Write to all sinks on the logger
  5. Sleep on condvar when idle
- `BackendOptions`: sleep duration, max batch size, queue capacity

#### 1.8 — ThreadContext & Frontend (`src/thread_context.rs`, `src/frontend.rs`)
- `ThreadContext`: thread-local holder of the SPSC queue + registration with backend
- `Frontend::create_or_get_logger(name, sinks)` — registry lookup or create
- `Frontend::preallocate()` — warm-up to avoid first-log allocation latency

#### 1.9 — rkyv-based log message (`src/message.rs`)
```rust
#[derive(Archive, Serialize)]
struct LogMessage {
    timestamp_ns: u64,
    metadata_ptr: usize,  // &'static Metadata as usize
    logger_ptr: usize,    // *const Logger as usize
    // Arguments serialized inline via rkyv as a flat byte slice
    args_data: Vec<u8>,
}
```
Hot path: `rkyv::to_bytes::<_, 256>(&msg)` — writes into stack buffer.  
Cold path: `unsafe { rkyv::access_unchecked::<ArchivedLogMessage>(&bytes) }`.

#### 1.10 — Declarative macros (`src/macros.rs`)
```rust
log_info!(logger, "Hello {}, value={}", name, x);
log_debug!(logger, "Debug: {:?}", obj);
log_warning!(logger, "Warn: {}", reason);
log_error!(logger, "Error code: {}", code);
log_critical!(logger, "Fatal: {}", detail);
```
Also: `log_trace_l1!`, `log_trace_l2!`, `log_trace_l3!`.

#### 1.11 — Integration test
- `N` threads × `M` messages, verify all appear in output, no data loss, timestamp ordering.

---

### Phase 2 — Unbounded Queue & More Sinks

- **2.1** Queue mode configuration (`FrontendOptions`): `UnboundedBlocking` (default), `UnboundedDropping`, `BoundedBlocking`, `BoundedDropping`
- **2.2** `FileSink`: buffered file I/O, append/truncate modes
- **2.3** `RotatingFileSink`: rotation by size (N MB) or time (hourly/daily), optional compression
- **2.4** `JsonFileSink` / `JsonConsoleSink`: structured JSON via `serde_json`
- **2.5** `NullSink`: no-op sink for benchmarking
- **2.6** Pattern formatter (`src/formatter.rs`): configurable `%Y-%m-%d %H:%M:%S.%f [%l] %n - %v` patterns

---

### Phase 3 — Advanced Features

- **3.1** Compile-time log level filtering (cargo features `max_level_*`)
- **3.2** Filters (`src/filter.rs`): `LevelFilter`, user-custom `Filter` trait
- **3.3** Backtrace logging (`src/backtrace.rs`): ring buffer of last N messages, auto-dump on error
- **3.4** Custom timestamp clocks: `SystemClock`, `TscClock` (RDTSC on x86), user `CustomClock`
- **3.5** Immediate flush: `logger.set_immediate_flush(true)`, compile-time disable option
- **3.6** Crash handler (`src/crash_handler.rs`): `signal-hook` for SIGSEGV/SIGABRT/SIGFPE/SIGINT, flushes all sinks

---

### Phase 4 — Custom DecodeFn Serialization

- **4.1** Proc macro `#[derive(LogArgs)]` generates optimal per-type `encode`/`decode` functions
- **4.2** `DecodeFn` function pointer pattern — type-erased decode per log site
- **4.3** Benchmark rkyv vs custom encoding, verify parity with Quill latency numbers

---

### Phase 5 — Performance & Polish

- **5.1** Huge pages support (Linux x86_64): `mmap(…, MAP_HUGETLB, …)` for queue storage
- **5.2** Preallocation API: `Frontend::preallocate()`
- **5.3** Criterion benchmarks: latency (50th/95th/99th), throughput (4M messages)
- **5.4** Documentation: rustdoc + mdbook quick start + cheat sheet
- **5.5** `log` crate facade (optional feature to use rapidlog as `log` backend)
- **5.6** Macro-free mode: `quill::info(logger, "msg {}", val)` function API (no macros)

---

## File Layout (target)

```
src/
├── lib.rs              # Public API re-exports
├── backend.rs          # BackendWorker, Backend::start()
├── frontend.rs         # Frontend, logger registry
├── logger.rs           # Logger struct
├── level.rs            # LogLevel enum
├── metadata.rs         # Metadata (static per call site)
├── message.rs          # rkyv LogMessage struct
├── queue.rs            # SPSC queue (rtrb wrapper + block chaining)
├── thread_context.rs   # ThreadContext (thread-local queue + state)
├── sink.rs             # Sink trait
├── formatter.rs        # PatternFormatter (Phase 2)
├── filter.rs           # Filter trait, LevelFilter (Phase 3)
├── timestamp.rs        # ClockSource, SystemClock, TscClock (Phase 3)
├── backtrace.rs        # Backtrace ring buffer (Phase 3)
├── crash_handler.rs    # Signal handler (Phase 3)
├── macros.rs           # Declarative logging macros
├── config.rs           # BackendOptions, FrontendOptions
└── sinks/
    ├── mod.rs
    ├── console.rs      # ConsoleSink (ANSI colors)
    ├── file.rs         # FileSink (Phase 2)
    ├── rotating.rs     # RotatingFileSink (Phase 2)
    ├── json.rs         # JsonFileSink / JsonConsoleSink (Phase 2)
    ├── null.rs         # NullSink (Phase 2)
    └── syslog.rs       # SyslogSink (Phase 2, cfg-gated)
```
