//! `log_error!`, `log_warn!`, `log_info!`, `log_debug!`:
//! `log_warn!("target", message, key = value, ...)`. The message is anything
//! `Display` (a literal or a `format!`); each value is `Display` too. Values
//! are only formatted when the level is on for the target.

macro_rules! log_at {
    ($level:ident, $target:expr, $msg:expr $(, $key:ident = $value:expr)* $(,)?) => {{
        let level = $crate::observability::log::Level::$level;
        let target: &str = $target;
        if $crate::observability::log::enabled(level, target) {
            $crate::observability::log::emit(
                level,
                target,
                &::std::string::ToString::to_string(&$msg),
                &[$((stringify!($key), ::std::string::ToString::to_string(&$value))),*],
            );
        }
    }};
}

macro_rules! log_error { ($($t:tt)*) => { log_at!(Error, $($t)*) } }
macro_rules! log_warn { ($($t:tt)*) => { log_at!(Warn, $($t)*) } }
macro_rules! log_info { ($($t:tt)*) => { log_at!(Info, $($t)*) } }
#[allow(unused_macros)]
macro_rules! log_debug { ($($t:tt)*) => { log_at!(Debug, $($t)*) } }
