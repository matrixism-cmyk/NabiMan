//! A small structured logger: one line per event with the time, level,
//! target, message and `key=value` fields; JSON lines instead with
//! `NABIMAN_LOG_FORMAT=json`. `NABIMAN_LOG` filters by level, for all
//! targets and per target: `info` (the default), `warn,mec=error`,
//! `debug,http=info`, `off`. Errors and warnings go to stderr, the rest to
//! stdout, as before (supervisor and journald keep them apart).
//!
//! Every message and value passes through `redact` first, so a token or a
//! password that slips into an error's text is not written out.

use super::redact;
use std::io::Write;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Level {
    Error = 1,
    Warn,
    Info,
    Debug,
}

impl Level {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Level::Error => "error",
            Level::Warn => "warn",
            Level::Info => "info",
            Level::Debug => "debug",
        }
    }

    /// `Some(None)` is `off`; `None` is not a level.
    fn parse(word: &str) -> Option<Option<Level>> {
        Some(Some(match word.trim().to_ascii_lowercase().as_str() {
            "error" => Level::Error,
            "warn" | "warning" => Level::Warn,
            "info" => Level::Info,
            "debug" | "trace" => Level::Debug,
            "off" | "none" => return Some(None),
            _ => return None,
        }))
    }
}

/// Which levels are on, overall and for targets (`mec` covers `mec::kube`).
#[derive(Debug, PartialEq)]
pub(crate) struct Filter {
    default: Option<Level>,
    targets: Vec<(String, Option<Level>)>,
}

impl Filter {
    /// `NABIMAN_LOG`'s value; parts that do not parse are skipped.
    pub(crate) fn parse(spec: &str) -> Self {
        let mut filter = Filter { default: Some(Level::Info), targets: Vec::new() };
        for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            match part.split_once('=') {
                Some((target, level)) => {
                    if let Some(level) = Level::parse(level) {
                        filter.targets.push((target.trim().to_string(), level));
                    }
                }
                None => {
                    if let Some(level) = Level::parse(part) {
                        filter.default = level;
                    }
                }
            }
        }
        // The longest (most specific) target first.
        filter.targets.sort_by_key(|(t, _)| std::cmp::Reverse(t.len()));
        filter
    }

    pub(crate) fn allows(&self, level: Level, target: &str) -> bool {
        let covers = |t: &str| {
            target == t || (target.starts_with(t) && target[t.len()..].starts_with(|c| matches!(c, ':' | '.' | '/')))
        };
        let max = self.targets.iter().find(|(t, _)| covers(t)).map_or(self.default, |(_, l)| *l);
        max.is_some_and(|max| level <= max)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Format {
    Text,
    Json,
}

struct Config {
    filter: Filter,
    format: Format,
}

fn config() -> &'static Config {
    static CONFIG: OnceLock<Config> = OnceLock::new();
    // First used after startup has applied the settings file to the environment.
    CONFIG.get_or_init(|| {
        let get = crate::install_config::values::env;
        Config {
            filter: Filter::parse(&get("NABIMAN_LOG").unwrap_or_default()),
            format: match get("NABIMAN_LOG_FORMAT").unwrap_or_default().trim().to_ascii_lowercase().as_str() {
                "json" => Format::Json,
                _ => Format::Text,
            },
        }
    })
}

pub(crate) fn enabled(level: Level, target: &str) -> bool {
    config().filter.allows(level, target)
}

pub(crate) fn emit(level: Level, target: &str, message: &str, fields: &[(&str, String)]) {
    let ts = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let mut line = format_line(&ts, level, target, message, fields, config().format);
    line.push('\n');
    // A full or closed stream loses the line; logging never stops the server.
    let _ = if level <= Level::Warn {
        std::io::stderr().lock().write_all(line.as_bytes())
    } else {
        std::io::stdout().lock().write_all(line.as_bytes())
    };
}

/// One event as a line (no newline), redacted.
pub(crate) fn format_line(ts: &str, level: Level, target: &str, message: &str, fields: &[(&str, String)], format: Format) -> String {
    let message = redact::text(message);
    let fields: Vec<(&str, String)> = fields.iter().map(|(k, v)| (*k, redact::field(k, v))).collect();
    match format {
        Format::Json => {
            let q = |s: &str| serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into());
            let mut out = format!(
                "{{\"ts\":{},\"level\":{},\"target\":{},\"msg\":{}",
                q(ts),
                q(level.name()),
                q(target),
                q(&message)
            );
            for (key, value) in &fields {
                let key = if matches!(*key, "ts" | "level" | "target" | "msg") { format!("field_{key}") } else { key.to_string() };
                out.push_str(&format!(",{}:{}", q(&key), q(value)));
            }
            out.push('}');
            out
        }
        Format::Text => {
            let mut out = format!("{ts} {:<5} {target}: {}", level.name().to_ascii_uppercase(), one_line(&message));
            for (key, value) in &fields {
                out.push(' ');
                out.push_str(key);
                out.push('=');
                out.push_str(&text_value(value));
            }
            out
        }
    }
}

/// A message on one line: a newline in it cannot forge a second event.
fn one_line(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\n', "\\n").replace('\r', "\\r")
}

/// A value as is when it is one plain word, else quoted and escaped.
fn text_value(v: &str) -> String {
    let plain = !v.is_empty() && v.chars().all(|c| !c.is_whitespace() && !c.is_control() && c != '"' && c != '=' && c != '\\');
    if plain { v.to_string() } else { serde_json::to_string(v).unwrap_or_else(|_| "\"\"".into()) }
}

#[cfg(test)]
#[path = "log_tests.rs"]
mod tests;
