//! The install's own settings: one optional file, read once at startup, and
//! the `NABIMAN_*` environment variables, which win over it.
//!
//! The file is TOML (`NABIMAN_CONFIG`, else `/etc/nabiman/nabiman.toml`,
//! else `<data dir>/nabiman.toml`). Each key names an environment variable:
//! `port = 18080` is `NABIMAN_PORT`, `[dictation] url = "…"` is
//! `NABIMAN_DICTATION_URL`, and a key already spelled `NABIMAN_…` is taken
//! as it is. Lists become comma-separated values.
//!
//! At startup, before any thread exists, every file setting the environment
//! does not already have is put into the process environment. So every
//! existing `std::env::var("NABIMAN_…")` reads env-then-file without change,
//! and an environment variable always overrides the file. The typed getters
//! in `values` add the defaults on top: environment > file > default.

mod catalogue;
mod file;
pub(crate) mod settings;
pub(crate) mod values;
#[cfg(test)]
mod docs_tests;
#[cfg(test)]
mod settings_tests;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub(crate) use catalogue::KNOWN;

/// The system-wide place for the file.
pub(crate) const DEFAULT_PATH: &str = "/etc/nabiman/nabiman.toml";

/// What startup took from the file: its path and the names it set (never values).
#[derive(Clone, Debug, Default, serde::Serialize)]
pub(crate) struct Loaded {
    pub file: Option<PathBuf>,
    /// Settings the file gave, which the environment did not override.
    pub from_file: Vec<String>,
    /// Settings the file gave but the environment already had (the environment won).
    pub overridden: Vec<String>,
    /// Names nothing in NabiMan reads (likely a typo); applied all the same.
    pub unknown: Vec<String>,
}

static LOADED: OnceLock<Loaded> = OnceLock::new();

/// What `apply_at_startup` did, or nothing when it did not run (tests).
pub(crate) fn loaded() -> Loaded {
    LOADED.get().cloned().unwrap_or_default()
}

/// Which file to read: `explicit` is `NABIMAN_CONFIG` (empty or `none`: no
/// file; a named file that is missing is an error), else the system path,
/// else the one in the data directory. `None`: there is no file.
pub(crate) fn locate(explicit: Option<&str>, system: &Path, data_dir: &Path) -> Result<Option<PathBuf>, String> {
    match explicit.map(str::trim) {
        Some("") | Some("none") => Ok(None),
        Some(named) => {
            let path = PathBuf::from(named);
            if path.exists() { Ok(Some(path)) } else { Err(format!("NABIMAN_CONFIG names {named}, which does not exist")) }
        }
        None => Ok([system.to_path_buf(), data_dir.join("nabiman.toml")].into_iter().find(|p| p.exists())),
    }
}

/// The value of `name`: the environment's (`env`), else the file's, else none —
/// what `plan` brings about in the process environment (the rule, for tests).
#[cfg(test)]
pub(crate) fn resolve(name: &str, env: &dyn Fn(&str) -> Option<String>, file: &BTreeMap<String, String>) -> Option<String> {
    env(name).or_else(|| file.get(name).cloned())
}

/// What applying `file` to an environment that has `env` gives: the
/// settings to set, and the report.
/// `in_data_dir`: the file is the one in the data directory.
pub(crate) fn plan(
    path: Option<PathBuf>,
    in_data_dir: bool,
    file: &BTreeMap<String, String>,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<(Vec<(String, String)>, Loaded), String> {
    let mut set = Vec::new();
    let mut report = Loaded { file: path, ..Loaded::default() };
    for (name, value) in file {
        if catalogue::not_from_file(name, in_data_dir) {
            return Err(format!("{name} cannot be set in this settings file; set it in the environment"));
        }
        if !KNOWN.contains(&name.as_str()) {
            report.unknown.push(name.clone());
        }
        if env(name).is_some() {
            report.overridden.push(name.clone());
        } else {
            set.push((name.clone(), value.clone()));
            report.from_file.push(name.clone());
        }
    }
    Ok((set, report))
}

/// Read the file and add its settings to the environment. Call once, first
/// thing in `main`, while the process has a single thread (setting the
/// environment is not safe once other threads may read it).
pub(crate) fn apply_at_startup() -> Result<(), String> {
    let explicit = std::env::var("NABIMAN_CONFIG").ok();
    let Some(path) = locate(explicit.as_deref(), Path::new(DEFAULT_PATH), &nabiman_server::data_dir())? else {
        let _ = LOADED.set(Loaded::default());
        return Ok(());
    };
    let settings = file::read(&path)?;
    let env = |name: &str| std::env::var(name).ok();
    let in_data_dir = path.starts_with(nabiman_server::data_dir());
    let (set, report) = plan(Some(path.clone()), in_data_dir, &settings, &env)
        .map_err(|e| format!("The settings file {}: {e}", path.display()))?;
    for (name, value) in &set {
        std::env::set_var(name, value);
    }
    println!("Settings: {} ({} from the file, {} overridden by the environment)", path.display(), report.from_file.len(), report.overridden.len());
    for name in &report.unknown {
        eprintln!("Settings: {name} in {} is not a setting NabiMan knows (a typo?)", path.display());
    }
    let _ = LOADED.set(report);
    Ok(())
}
