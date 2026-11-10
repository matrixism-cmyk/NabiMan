//! Typed reads of settings. Each `*_in` takes the lookup to use, so tests
//! need not touch the process environment; the plain forms read the
//! environment, which at startup took in the settings file (see `super`).

use std::path::{Path, PathBuf};
use std::str::FromStr;

/// Where a setting's raw value comes from.
pub(crate) type Lookup<'a> = &'a dyn Fn(&str) -> Option<String>;

/// The process environment (which holds the file's settings after startup).
pub(crate) fn env(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// A setting's text, trimmed; set to nothing is the same as not set.
pub(crate) fn text_in(get: Lookup, name: &str) -> Option<String> {
    get(name).map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// A number; one that does not parse is the default.
pub(crate) fn number_in<T: FromStr>(get: Lookup, name: &str, default: T) -> T {
    text_in(get, name).and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// A comma-separated list, each item trimmed, empty ones dropped.
pub(crate) fn list_in(get: Lookup, name: &str) -> Vec<String> {
    text_in(get, name)
        .map(|v| v.split(',').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect())
        .unwrap_or_default()
}

/// A file that can be run.
pub(crate) fn runnable(path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.mode() & 0o111 != 0)
}

/// The first of `names` found in the directories of `path_var` (a `PATH`
/// value), then the first of `places` that can be run.
pub(crate) fn find_program(names: &[&str], path_var: &str, places: &[PathBuf]) -> Option<PathBuf> {
    let on_path = path_var
        .split(':')
        .filter(|d| d.starts_with('/'))
        .flat_map(|dir| names.iter().map(move |n| Path::new(dir).join(n)));
    on_path.chain(places.iter().cloned()).find(|p| runnable(p))
}

/// A program setting: the path it names, if set (even when nothing is
/// there, so the caller can say so), else what `find_program` finds.
pub(crate) fn program_in(get: Lookup, name: &str, names: &[&str], places: &[PathBuf]) -> Option<PathBuf> {
    match text_in(get, name) {
        Some(set) => Some(PathBuf::from(set)),
        None => find_program(names, &get("PATH").unwrap_or_default(), places),
    }
}
