//! How much the data directory holds, walked without following links and
//! with a ceiling on entries (a hosted instance's storage line on the bill).

use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

const MAX_ENTRIES: usize = 200_000;
const MAX_DEPTH: usize = 24;
const TOP: usize = 8;

#[derive(Serialize, Debug, Default, PartialEq)]
pub(crate) struct Part {
    pub name: String,
    pub bytes: u64,
}

#[derive(Serialize, Debug, Default, PartialEq)]
pub(crate) struct Storage {
    /// File sizes added up (bytes).
    pub bytes: u64,
    pub files: u64,
    /// False when the walk stopped at its entry ceiling or met unreadable directories.
    pub complete: bool,
    /// The largest top-level entries of the data directory.
    pub largest: Vec<Part>,
}

pub(crate) fn measure(dir: &Path) -> Storage {
    let mut out = Storage { complete: true, ..Default::default() };
    let mut parts: HashMap<String, u64> = HashMap::new();
    // (path, depth, top-level name)
    let mut stack = vec![(dir.to_path_buf(), 0usize, None::<String>)];
    let mut seen = 0usize;
    while let Some((path, depth, top)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&path) else {
            out.complete = false;
            continue;
        };
        for entry in entries.flatten() {
            seen += 1;
            if seen > MAX_ENTRIES {
                out.complete = false;
                stack.clear();
                break;
            }
            let Ok(meta) = std::fs::symlink_metadata(entry.path()) else { continue };
            let name = top.clone().unwrap_or_else(|| entry.file_name().to_string_lossy().into_owned());
            if meta.is_dir() {
                if depth + 1 < MAX_DEPTH {
                    stack.push((entry.path(), depth + 1, Some(name)));
                } else {
                    out.complete = false;
                }
            } else if meta.is_file() {
                out.bytes += meta.len();
                out.files += 1;
                *parts.entry(name).or_default() += meta.len();
            }
        }
    }
    let mut largest: Vec<Part> = parts.into_iter().map(|(name, bytes)| Part { name, bytes }).collect();
    largest.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.name.cmp(&b.name)));
    largest.truncate(TOP);
    out.largest = largest;
    out
}
