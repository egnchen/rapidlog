#[macro_export]
macro_rules! log_trace_l3 {
    ($logger:expr, $($arg:tt)*) => {
        {
            const _META: &$crate::metadata::Metadata = &$crate::metadata::Metadata::new(
                $crate::level::LogLevel::TraceL3,
                stringify!($($arg)*),
                file!(),
                line!(),
                module_path!(),
            );
            #[allow(unused_unsafe)]
            if unsafe { ($crate::logger::Logger::log_level(&*std::sync::Arc::as_ptr(&($logger)))) }.as_usize() <= $crate::level::LogLevel::TraceL3.as_usize() {
                let _formatted = format!($($arg)*);
                let _msg = $crate::message::LogMessage::new(
                    0u64,
                    _META,
                    std::sync::Arc::as_ptr(&($logger)) as *const $crate::logger::Logger,
                    _formatted.into_bytes(),
                );
                if let Ok(_encoded) = _msg.encode() {
                    $crate::thread_context::ThreadContext::push(&_encoded).ok();
                }
            }
        }
    };
}

#[macro_export]
macro_rules! log_trace_l2 {
    ($logger:expr, $($arg:tt)*) => {
        {
            const _META: &$crate::metadata::Metadata = &$crate::metadata::Metadata::new(
                $crate::level::LogLevel::TraceL2,
                stringify!($($arg)*),
                file!(),
                line!(),
                module_path!(),
            );
            #[allow(unused_unsafe)]
            if unsafe { ($crate::logger::Logger::log_level(&*std::sync::Arc::as_ptr(&($logger)))) }.as_usize() <= $crate::level::LogLevel::TraceL2.as_usize() {
                let _formatted = format!($($arg)*);
                let _msg = $crate::message::LogMessage::new(
                    0u64,
                    _META,
                    std::sync::Arc::as_ptr(&($logger)) as *const $crate::logger::Logger,
                    _formatted.into_bytes(),
                );
                if let Ok(_encoded) = _msg.encode() {
                    $crate::thread_context::ThreadContext::push(&_encoded).ok();
                }
            }
        }
    };
}

#[macro_export]
macro_rules! log_trace_l1 {
    ($logger:expr, $($arg:tt)*) => {
        {
            const _META: &$crate::metadata::Metadata = &$crate::metadata::Metadata::new(
                $crate::level::LogLevel::TraceL1,
                stringify!($($arg)*),
                file!(),
                line!(),
                module_path!(),
            );
            #[allow(unused_unsafe)]
            if unsafe { ($crate::logger::Logger::log_level(&*std::sync::Arc::as_ptr(&($logger)))) }.as_usize() <= $crate::level::LogLevel::TraceL1.as_usize() {
                let _formatted = format!($($arg)*);
                let _msg = $crate::message::LogMessage::new(
                    0u64,
                    _META,
                    std::sync::Arc::as_ptr(&($logger)) as *const $crate::logger::Logger,
                    _formatted.into_bytes(),
                );
                if let Ok(_encoded) = _msg.encode() {
                    $crate::thread_context::ThreadContext::push(&_encoded).ok();
                }
            }
        }
    };
}

#[macro_export]
macro_rules! log_debug {
    ($logger:expr, $($arg:tt)*) => {
        {
            const _META: &$crate::metadata::Metadata = &$crate::metadata::Metadata::new(
                $crate::level::LogLevel::Debug,
                stringify!($($arg)*),
                file!(),
                line!(),
                module_path!(),
            );
            #[allow(unused_unsafe)]
            if unsafe { ($crate::logger::Logger::log_level(&*std::sync::Arc::as_ptr(&($logger)))) }.as_usize() <= $crate::level::LogLevel::Debug.as_usize() {
                let _formatted = format!($($arg)*);
                let _msg = $crate::message::LogMessage::new(
                    0u64,
                    _META,
                    std::sync::Arc::as_ptr(&($logger)) as *const $crate::logger::Logger,
                    _formatted.into_bytes(),
                );
                if let Ok(_encoded) = _msg.encode() {
                    $crate::thread_context::ThreadContext::push(&_encoded).ok();
                }
            }
        }
    };
}

#[macro_export]
macro_rules! log_info {
    ($logger:expr, $($arg:tt)*) => {
        {
            const _META: &$crate::metadata::Metadata = &$crate::metadata::Metadata::new(
                $crate::level::LogLevel::Info,
                stringify!($($arg)*),
                file!(),
                line!(),
                module_path!(),
            );
            #[allow(unused_unsafe)]
            if unsafe { ($crate::logger::Logger::log_level(&*std::sync::Arc::as_ptr(&($logger)))) }.as_usize() <= $crate::level::LogLevel::Info.as_usize() {
                let _formatted = format!($($arg)*);
                let _msg = $crate::message::LogMessage::new(
                    0u64,
                    _META,
                    std::sync::Arc::as_ptr(&($logger)) as *const $crate::logger::Logger,
                    _formatted.into_bytes(),
                );
                if let Ok(_encoded) = _msg.encode() {
                    $crate::thread_context::ThreadContext::push(&_encoded).ok();
                }
            }
        }
    };
}

#[macro_export]
macro_rules! log_warning {
    ($logger:expr, $($arg:tt)*) => {
        {
            const _META: &$crate::metadata::Metadata = &$crate::metadata::Metadata::new(
                $crate::level::LogLevel::Warning,
                stringify!($($arg)*),
                file!(),
                line!(),
                module_path!(),
            );
            #[allow(unused_unsafe)]
            if unsafe { ($crate::logger::Logger::log_level(&*std::sync::Arc::as_ptr(&($logger)))) }.as_usize() <= $crate::level::LogLevel::Warning.as_usize() {
                let _formatted = format!($($arg)*);
                let _msg = $crate::message::LogMessage::new(
                    0u64,
                    _META,
                    std::sync::Arc::as_ptr(&($logger)) as *const $crate::logger::Logger,
                    _formatted.into_bytes(),
                );
                if let Ok(_encoded) = _msg.encode() {
                    $crate::thread_context::ThreadContext::push(&_encoded).ok();
                }
            }
        }
    };
}

#[macro_export]
macro_rules! log_error {
    ($logger:expr, $($arg:tt)*) => {
        {
            const _META: &$crate::metadata::Metadata = &$crate::metadata::Metadata::new(
                $crate::level::LogLevel::Error,
                stringify!($($arg)*),
                file!(),
                line!(),
                module_path!(),
            );
            #[allow(unused_unsafe)]
            if unsafe { ($crate::logger::Logger::log_level(&*std::sync::Arc::as_ptr(&($logger)))) }.as_usize() <= $crate::level::LogLevel::Error.as_usize() {
                let _formatted = format!($($arg)*);
                let _msg = $crate::message::LogMessage::new(
                    0u64,
                    _META,
                    std::sync::Arc::as_ptr(&($logger)) as *const $crate::logger::Logger,
                    _formatted.into_bytes(),
                );
                if let Ok(_encoded) = _msg.encode() {
                    $crate::thread_context::ThreadContext::push(&_encoded).ok();
                }
            }
        }
    };
}

#[macro_export]
macro_rules! log_critical {
    ($logger:expr, $($arg:tt)*) => {
        {
            const _META: &$crate::metadata::Metadata = &$crate::metadata::Metadata::new(
                $crate::level::LogLevel::Critical,
                stringify!($($arg)*),
                file!(),
                line!(),
                module_path!(),
            );
            #[allow(unused_unsafe)]
            if unsafe { ($crate::logger::Logger::log_level(&*std::sync::Arc::as_ptr(&($logger)))) }.as_usize() <= $crate::level::LogLevel::Critical.as_usize() {
                let _formatted = format!($($arg)*);
                let _msg = $crate::message::LogMessage::new(
                    0u64,
                    _META,
                    std::sync::Arc::as_ptr(&($logger)) as *const $crate::logger::Logger,
                    _formatted.into_bytes(),
                );
                if let Ok(_encoded) = _msg.encode() {
                    $crate::thread_context::ThreadContext::push(&_encoded).ok();
                }
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use crate::logger::Logger;
    use crate::sinks::ConsoleSink;
    use std::sync::Arc;

    #[test]
    fn log_info_macro_compiles() {
        let logger = Logger::new("macro_test".to_string(), vec![Arc::new(ConsoleSink::new())]);
        log_info!(logger, "hello {}", "world");
    }

    #[test]
    fn log_levels_all_compile() {
        let logger = Logger::new("all_levels".to_string(), vec![Arc::new(ConsoleSink::new())]);
        logger.set_log_level(crate::level::LogLevel::TraceL3);

        log_trace_l3!(logger, "trace_l3: {}", 1);
        log_trace_l2!(logger, "trace_l2: {}", 2);
        log_trace_l1!(logger, "trace_l1: {}", 3);
        log_debug!(logger, "debug: {}", 4);
        log_info!(logger, "info: {}", 5);
        log_warning!(logger, "warning: {}", 6);
        log_error!(logger, "error: {}", 7);
        log_critical!(logger, "critical: {}", 8);
    }

    #[test]
    fn log_level_filtering() {
        let logger = Logger::new("filter".to_string(), vec![Arc::new(ConsoleSink::new())]);
        logger.set_log_level(crate::level::LogLevel::Warning);
        // These should compile and not panic (just skip due to level check)
        log_debug!(logger, "this should be filtered out");
        log_info!(logger, "this too");
        log_warning!(logger, "this should pass: {}", 42);
    }
}
