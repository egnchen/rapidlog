# Rapidlog Agent Instructions

## Project

Rapidlog is a **high-performance asynchronous logging library for Rust**, ported from the [Quill C++ logging library](https://github.com/odygrd/quill).

**Architecture:** Caller threads push encoded log messages into per-thread lock-free SPSC queues. A single backend worker thread pops all queues, sorts messages by timestamp, formats them, and dispatches to output sinks (console, file, JSON, etc.). This keeps the hot path — the caller's `log_info!()` call — as fast as possible (target: single-digit nanoseconds).

**Tech stack:** Rust edition 2024, `rtrb` (SPSC ring buffer), `rkyv = "0.7"` (zero-copy serialization), `parking_lot`, `anstream` (colored console), `is-terminal`, `time`.

See `PLAN.md` for the full implementation plan.

## Ralph-Loop Prompt

You are an autonomous coding agent implementing a Rust logging library. Work through the TODO list below **one item at a time**, from top to bottom.

### Instructions (per iteration)

1. Read `PLAN.md` and `progress.txt` (check **Codebase Patterns** section first) to orient yourself.
2. Pick the **next unchecked item** from the TODO list below.
3. Implement it. Read any existing source files first. Follow the conventions established in already-written code.
4. Run quality checks:
   - `cargo check` — must pass
   - `cargo test` — must pass
   - `cargo clippy -- -D warnings` — must pass (if clippy configured)
   - `cargo fmt -- --check` — must pass (if rustfmt configured)
5. If checks fail, fix the issues and re-run until green. Do NOT proceed with failures.
6. Update the TODO list: mark the completed item `[x]` and add any new follow-up items discovered during implementation.
7. Append your progress to `progress.txt` using the format below. Do NOT replace — always append.
8. Commit your changes with a concise message referencing the task: `feat(task-id): description`
9. If there are remaining unchecked items, end your response normally. If ALL items are `[x]`, respond with: `<promise>COMPLETE</promise>`

### Progress Format (append to progress.txt)

```
## [Date/Time] — [Task ID]
- What was done
- Files changed/created
- **Learnings for future iterations:**
  - Patterns discovered
  - Gotchas encountered
  - Useful context
---
```

### Consolidate Patterns

If you discover a **reusable pattern**, add it to the `## Codebase Patterns` section at the TOP of `progress.txt`:

```
## Codebase Patterns
- Pattern description
- Another pattern
```

Only add patterns that are **general and reusable** — not task-specific details.

### Quality Rules

- NEVER commit broken code
- Keep changes focused and minimal
- Follow existing code patterns
- Do not add comments unless asked
- Do not create documentation files unless asked

---

## TODO List — Phase 1 (Core Foundation MVP)

Goal: async console logging working end-to-end.

| # | Done | Task | Depends On |
|---|------|------|------------|
| 1.1 | [x] | **Project scaffold** — Add dependencies to `Cargo.toml` (`rtrb`, `rkyv = "0.7"`, `parking_lot`, `anstream`, `is-terminal`, `time`, `thiserror`). Create `src/lib.rs` with `mod` declarations for all planned modules (stub files). | — |
| 1.2 | [x] | **LogLevel + Metadata** — `src/level.rs`: `LogLevel` enum (TraceL3..Critical) with `as_usize()`/`from_usize()`. `src/metadata.rs`: `Metadata` struct holding `level`, `format_str`, `file`, `line`, `module_path` — all `&'static str`. | — |
| 1.3 | [x] | **SPSC Queue** — `src/queue.rs`: wrap `rtrb::RingBuffer<u8>` with length-prefixed message framing (write `u32` length + payload). Expose `push(&[u8]) -> Result<(), QueueFull>` and `pop() -> Option<Vec<u8>>`. Include a stub for unbounded block chaining (Phase 2). | — |
| 1.4 | [x] | **Sink trait + ConsoleSink** — `src/sink.rs`: `trait Sink: Send + Sync { fn write(&self, formatted: &str); fn flush(&self); }`. `src/sinks/console.rs`: `ConsoleSink` with ANSI coloring via `anstream`, TTY detection via `is-terminal`. `src/sinks/mod.rs` re-exports. | — |
| 1.5 | [x] | **Logger** — `src/logger.rs`: `Logger` struct with `name: String`, `log_level: AtomicU8`, `sinks: Vec<Arc<dyn Sink>>`. Global registry via `parking_lot::RwLock<HashMap<String, Arc<Logger>>>`. `Logger::new(name, sinks)` => create-or-get. `set_log_level(level)`. | 1.2, 1.4 |
| 1.6 | [x] | **LogMessage (rkyv)** — `src/message.rs`: `LogMessage` struct with `timestamp_ns: u64`, `metadata_ptr: usize` (pointer to `&'static Metadata`), `logger_ptr: usize` (pointer to `*const Logger`), `args_data: Vec<u8>`. Derive `rkyv::Archive`, `rkyv::Serialize`. Encode/decode helpers. | 1.2, 1.5 |
| 1.7 | [x] | **ThreadContext** — `src/thread_context.rs`: thread-local holder containing the SPSC queue (via `std::cell::RefCell` + `thread_local!`). `ThreadContext::with(|ctx| ctx.push(data))` for the hot path. Registration with backend for polling. | 1.3 |
| 1.8 | [x] | **Backend worker** — `src/backend.rs`: `Backend::start(options)` spawns single worker thread. Loop: poll all registered `ThreadContext` queues → accumulate batch → sort by timestamp → for each message, zero-copy access `ArchivedLogMessage` via rkyv, format with `std::fmt`, dispatch to all logger sinks. Sleep on `parking_lot::Condvar` when idle. `BackendOptions` struct. | 1.3, 1.4, 1.5, 1.6, 1.7 |
| 1.9 | [x] | **Frontend** — `src/frontend.rs`: `Frontend` static methods. `create_or_get_logger(name, sinks)` — delegates to registry. `preallocate()` — warms up thread-local queue. Registers thread context with backend on first use. | 1.5, 1.7, 1.8 |
| 1.10 | [x] | **Logging macros** — `src/macros.rs`: declarative macros for each log level (`log_trace_l3!`, `log_trace_l2!`, `log_trace_l1!`, `log_debug!`, `log_info!`, `log_warning!`, `log_error!`, `log_critical!`). Each: create `Metadata` as `const`, encode `LogMessage` via rkyv, push to thread-local queue. | 1.2, 1.6, 1.7, 1.8 |
| 1.11 | [x] | **Public API + lib.rs** — `src/lib.rs`: re-export `LogLevel`, `Logger`, `Sink`, `ConsoleSink`, `Backend`, `BackendOptions`, `Frontend`, and all macros. Public API surface: everything needed for quick-start. | 1.2–1.10 |
| 1.12 | [x] | **Integration test** — `tests/integration.rs`: spawn N threads, each logs M messages at various levels. Flush backend. Assert: all messages received, no data loss, timestamp ordering preserved. | 1.1–1.11 |

---

## TODO List — Phase 2 (Zero-Allocation Hot Path)

Goal: eliminate all heap allocations from the hot path. Defer formatting to backend. Single-digit nanosecond latency target.

| # | Done | Task | Depends On |
|---|------|------|------------|
| 2.0 | [x] | **`LogArg` trait + encoding (`src/arg.rs`)** — Trait with `log_tag()`, `log_encode()`, `log_max_size()`. Impls for `i32, i64, u32, u64, usize, f32, f64, bool, &str, String`. Packed layout: count byte + all tags + all payloads (no interleaving). `DecodedArg` enum with `as_display_string()`. Single generic `decode_args()` function using tag-based dispatch. `format_with_args()` for backend formatting. `DisplayArg<T>` / `DebugArg<T>` wrappers for fallback types. | 1.2 |
| 2.1 | [x] | **Ring buffer 8-byte header (`src/queue.rs`)** — Changed from 4-byte to 8-byte length-prefix header (4 bytes length LE + 4 bytes flags) for 8-byte alignment. `push()` writes zero-initialized chunk, then header + data. `pop()` reads 8-byte header and extracts payload. | 1.3 |
| 2.2 | [x] | **ThreadContext unchanged** — Existing `push()` method is sufficient. Stack buffer encoding feeds into it. | 1.7 |
| 2.3 | [x] | **Refactor `LogMessage` (`src/message.rs`)** — Replaced `args_data: Vec<u8>` with `args_len: u16` (inline arg data follows archive header). `serialize_header_into(&self, buf)` writes raw bytes matching rkyv archive layout (32 bytes: 3×u64 + u16 + padding). `decode()` uses raw pointer cast to `&ArchivedLogMessage` (no validation). | 1.6 |
| 2.4 | [x] | **Rewrite macros (`src/macros.rs`)** — New pattern: `($logger, $fmt:literal $(, $arg:expr)*)`. Hot path: compute total size from `log_max_size()` calls, use 512-byte stack buffer (zero alloc) or `vec!` fallback for large messages. Encode: header (via `serialize_header_into`) + args (count + packed tags + payloads). Level check unchanged. All 8 log levels updated. | 2.0, 2.1, 2.3 |
| 2.5 | [x] | **Update backend (`src/backend.rs`)** — `format_message()` now reads `archived.args_len`, extracts inline arg bytes after archive header, calls `decode_args()` → `format_with_args()`. Timestamp prefix + level + file:line output unchanged. | 2.0, 2.3 |
| 2.6 | [x] | **Integration tests + benchmarks** — Updated tests for new macro signature (`"fmt {}", arg` not `"fmt {arg}"`). Criterion benchmarks: 1 integer (10.4 ns), 2 floats (11.5 ns), 3 strings (15.3 ns), 1 Vec<String> via DebugArg (204 ns). Speedup: 11-22× for primitives, 1.5× for complex types. | 2.0–2.5 |

**Remaining optimization notes:**

- The `drain_queue()` call inside `b.iter_custom` is included in benchmark timing, inflating results slightly (~2-3 ns per message batch)
- `Vec<String>` benchmark uses `DebugArg` wrapper which calls `format!("{:?}")` internally — one heap allocation remains
- Stack buffer threshold (512 bytes) works for most log calls. Larger messages fall back to `Vec<u8>` (one allocation)
- Timestamp is currently hardcoded to 0 — adding a monotonic clock (e.g. TSC) would add ~5-15 ns
- Direct ring buffer write (no stack intermediary copy) could save another ~2-5 ns per call

---

## Commands

```bash
cargo check              # Type check
cargo test               # Run all tests
cargo clippy -- -D warnings  # Lint (strict)
cargo fmt -- --check     # Format check
cargo build --release    # Release build
```
