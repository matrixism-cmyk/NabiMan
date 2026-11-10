//! Reading the settings file: who may write it, and its TOML as `NABIMAN_*` names.

use std::collections::BTreeMap;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

/// Larger than any real settings file.
const MAX_BYTES: u64 = 256 << 10;

/// The file at `path` as `NABIMAN_*` name → value, refused when someone
/// other than root or this process's user could change it (it names programs
/// NabiMan runs) or when it is not valid.
pub(crate) fn read(path: &Path) -> Result<BTreeMap<String, String>, String> {
    let shown = path.display();
    let meta = std::fs::metadata(path).map_err(|e| format!("Cannot read the settings file {shown}: {e}"))?;
    // SAFETY: geteuid has no preconditions.
    let me = unsafe { libc::geteuid() };
    check_owner(meta.is_file(), meta.uid(), meta.mode(), me).map_err(|why| format!("The settings file {shown} {why}"))?;
    if meta.len() > MAX_BYTES {
        return Err(format!("The settings file {shown} is larger than {} KiB", MAX_BYTES >> 10));
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("Cannot read the settings file {shown}: {e}"))?;
    parse(&text).map_err(|e| format!("The settings file {shown}: {e}"))
}

/// Why a file with this owner and mode may not be trusted, if it may not.
pub(crate) fn check_owner(is_file: bool, uid: u32, mode: u32, me: u32) -> Result<(), String> {
    if !is_file {
        return Err("is not a regular file".into());
    }
    if uid != 0 && uid != me {
        return Err(format!("belongs to uid {uid}; it should belong to root"));
    }
    if mode & 0o022 != 0 {
        return Err(format!("can be changed by other users (mode {:o}); run chmod go-w on it", mode & 0o777));
    }
    Ok(())
}

/// The TOML text as `NABIMAN_*` name → value.
pub(crate) fn parse(text: &str) -> Result<BTreeMap<String, String>, String> {
    let value: toml::Value = text.parse().map_err(|e: toml::de::Error| e.to_string())?;
    let toml::Value::Table(table) = value else { return Err("not a table".into()) };
    let mut out = BTreeMap::new();
    flatten(&table, &mut Vec::new(), &mut out)?;
    Ok(out)
}

fn name_of(path: &[String]) -> Result<String, String> {
    if let [only] = path {
        if only.starts_with("NABIMAN_") && only.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_') {
            return Ok(only.clone());
        }
    }
    let mut name = String::from("NABIMAN");
    for part in path {
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') {
            return Err(format!("{:?} is not a setting name (letters, digits, _ and -)", path.join(".")));
        }
        name.push('_');
        name.push_str(&part.replace('-', "_").to_ascii_uppercase());
    }
    Ok(name)
}

fn scalar(value: &toml::Value) -> Option<String> {
    match value {
        toml::Value::String(s) => Some(s.clone()),
        toml::Value::Integer(i) => Some(i.to_string()),
        toml::Value::Float(f) => Some(f.to_string()),
        toml::Value::Boolean(b) => Some(b.to_string()),
        toml::Value::Datetime(d) => Some(d.to_string()),
        toml::Value::Array(_) | toml::Value::Table(_) => None,
    }
}

fn flatten(table: &toml::value::Table, path: &mut Vec<String>, out: &mut BTreeMap<String, String>) -> Result<(), String> {
    for (key, value) in table {
        path.push(key.clone());
        match value {
            toml::Value::Table(inner) => flatten(inner, path, out)?,
            toml::Value::Array(items) => {
                let parts: Option<Vec<String>> = items.iter().map(scalar).collect();
                let parts = parts.ok_or_else(|| format!("{} holds a list of tables or lists; only plain values", path.join(".")))?;
                if parts.iter().any(|p| p.contains(',')) {
                    return Err(format!("{}: a list item may not contain a comma", path.join(".")));
                }
                insert(out, name_of(path)?, parts.join(","))?;
            }
            plain => insert(out, name_of(path)?, scalar(plain).unwrap_or_default())?,
        }
        path.pop();
    }
    Ok(())
}

fn insert(out: &mut BTreeMap<String, String>, name: String, value: String) -> Result<(), String> {
    if value.contains('\0') {
        return Err(format!("{name}: a value may not contain a NUL character"));
    }
    match out.insert(name.clone(), value) {
        Some(_) => Err(format!("{name} is set twice")),
        None => Ok(()),
    }
}
