#[doc(hidden)]
#[macro_export]
macro_rules! log_impl {
    ($level:expr, $logger:expr, $fmt:literal $(, $arg:expr)* $(,)?) => {
        {
            const _META: &$crate::metadata::Metadata = &$crate::metadata::Metadata::new(
                $level,
                $fmt,
                file!(),
                line!(),
                module_path!(),
            );
            #[allow(unused_unsafe)]
            if unsafe { ($crate::logger::Logger::log_level(&*std::sync::Arc::as_ptr(&($logger)))) }.as_usize() <= $level.as_usize() {
                let _arg_count: usize = 0 $(+ { let _ = &$arg; 1 })*;
                let _tags_size = 1 + _arg_count;
                let _payloads_size: usize = 0 $(+ $crate::arg::LogArg::log_max_size(&$arg))*;
                let _total_args = _tags_size + _payloads_size;
                let _total_msg = $crate::message::ARCHIVED_HEADER_SIZE + _total_args;

                $crate::thread_context::ThreadContext::push_encoded(_total_msg, |__buf| {
                    let msg = $crate::message::LogMessage::new(
                        // TODO(tsc): replace 0u64 with rdtsc() for nanosecond timestamps.
                        // Expected cost: ~5-15ns. Gate behind cfg(target_arch = "x86_64") feature.
                        0u64, _META,
                        std::sync::Arc::as_ptr(&($logger)) as *const $crate::logger::Logger,
                        _total_args as u16,
                    );
                    msg.serialize_header_into(&mut __buf[..$crate::message::ARCHIVED_HEADER_SIZE]);
                    __buf[$crate::message::ARCHIVED_HEADER_SIZE] = _arg_count as u8;
                    let mut __tp = $crate::message::ARCHIVED_HEADER_SIZE + 1;
                    let mut __dp = $crate::message::ARCHIVED_HEADER_SIZE + 1 + _arg_count;
                    $({
                        let __a = &$arg;
                        __buf[__tp] = $crate::arg::LogArg::log_tag(__a);
                        __tp += 1;
                        let __w = $crate::arg::LogArg::log_encode(__a, &mut __buf[__dp..]);
                        __dp += __w;
                    })*
                }).ok();
            }
        }
    };
}

#[macro_export]
macro_rules! log_trace_l3 {
    ($logger:expr, $fmt:literal $(, $arg:expr)* $(,)?) => {
        $crate::log_impl!($crate::level::LogLevel::TraceL3, $logger, $fmt $(, $arg)*)
    };
}

#[macro_export]
macro_rules! log_trace_l2 {
    ($logger:expr, $fmt:literal $(, $arg:expr)* $(,)?) => {
        $crate::log_impl!($crate::level::LogLevel::TraceL2, $logger, $fmt $(, $arg)*)
    };
}

#[macro_export]
macro_rules! log_trace_l1 {
    ($logger:expr, $fmt:literal $(, $arg:expr)* $(,)?) => {
        $crate::log_impl!($crate::level::LogLevel::TraceL1, $logger, $fmt $(, $arg)*)
    };
}

#[macro_export]
macro_rules! log_debug {
    ($logger:expr, $fmt:literal $(, $arg:expr)* $(,)?) => {
        $crate::log_impl!($crate::level::LogLevel::Debug, $logger, $fmt $(, $arg)*)
    };
}

#[macro_export]
macro_rules! log_info {
    ($logger:expr, $fmt:literal $(, $arg:expr)* $(,)?) => {
        $crate::log_impl!($crate::level::LogLevel::Info, $logger, $fmt $(, $arg)*)
    };
}

#[macro_export]
macro_rules! log_warning {
    ($logger:expr, $fmt:literal $(, $arg:expr)* $(,)?) => {
        $crate::log_impl!($crate::level::LogLevel::Warning, $logger, $fmt $(, $arg)*)
    };
}

#[macro_export]
macro_rules! log_error {
    ($logger:expr, $fmt:literal $(, $arg:expr)* $(,)?) => {
        $crate::log_impl!($crate::level::LogLevel::Error, $logger, $fmt $(, $arg)*)
    };
}

#[macro_export]
macro_rules! log_critical {
    ($logger:expr, $fmt:literal $(, $arg:expr)* $(,)?) => {
        $crate::log_impl!($crate::level::LogLevel::Critical, $logger, $fmt $(, $arg)*)
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
    fn log_info_no_args() {
        let logger = Logger::new(
            "macro_no_args".to_string(),
            vec![Arc::new(ConsoleSink::new())],
        );
        log_info!(logger, "hello world");
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
        log_debug!(logger, "this should be filtered out");
        log_info!(logger, "this too");
        log_warning!(logger, "this should pass: {}", 42);
    }

    #[test]
    fn log_with_multiple_args() {
        let logger = Logger::new("multi_args".to_string(), vec![Arc::new(ConsoleSink::new())]);
        logger.set_log_level(crate::level::LogLevel::TraceL3);
        log_info!(logger, "a: {}, b: {}, c: {}", 1, 2.5, "three");
    }
}
