#![allow(unsafe_op_in_unsafe_fn)]

pub mod arg;
pub mod backend;
pub mod filter;
pub mod frontend;
pub mod level;
pub mod logger;
pub mod macros;
pub mod message;
pub mod metadata;
pub mod queue;
pub mod sink;
pub mod sinks;
pub mod thread_context;
pub mod timestamp;

pub use backend::{Backend, BackendHandle, BackendOptions};
pub use filter::{Filter, LevelFilter};
pub use frontend::Frontend;
pub use level::LogLevel;
pub use logger::Logger;
pub use metadata::Metadata;
pub use sink::Sink;
pub use sinks::ConsoleSink;
pub use timestamp::{now, to_display_nanos};
