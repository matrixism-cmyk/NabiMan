//! The model that last answered in a Claude Code session, read from the end
//! of its transcript.

use crate::lock_safe::LockExt;
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// A model name as transcripts and the pages may carry it: letters, digits, `-`, `.`, `[`, `]`.
pub(crate) fn model_ok(m: &str) -> bool {
    !m.is_empty() && m.len() <= 60 && m.chars().all(|c| c.is_ascii_alphanumeric() || "-._[]".contains(c))
}

/// The last `"model":"…"` in a piece of a transcript; synthetic entries are passed over.
pub(crate) fn model_in(text: &str) -> Option<String> {
    const KEY: &str = "\"model\":\"";
    let mut rest = text;
    let mut found = None;
    while let Some(i) = rest.find(KEY) {
        let after = &rest[i + KEY.len()..];
        let end = after.find('"')?;
        let m = &after[..end];
        if model_ok(m) && !m.starts_with('<') && m != "synthetic" {
            found = Some(m.to_string());
        }
        rest = &after[end..];
    }
    found
}

type ModelCache = Mutex<Option<HashMap<PathBuf, (u64, Option<String>)>>>;
static MODELS: ModelCache = Mutex::new(None);

/// The model that last answered in a transcript, read from its end (and again only when it has grown).
pub(super) fn last_model(transcript: &Path) -> Option<String> {
    let len = fs::metadata(transcript).ok()?.len();
    let mut guard = MODELS.lock_safe();
    let cache = guard.get_or_insert_with(HashMap::new);
    if let Some((at, model)) = cache.get(transcript) {
        if *at == len {
            return model.clone();
        }
    }
    let mut file = fs::File::open(transcript).ok()?;
    let from = len.saturating_sub(512 * 1024);
    std::io::Seek::seek(&mut file, std::io::SeekFrom::Start(from)).ok()?;
    let mut raw = Vec::new();
    file.take(512 * 1024).read_to_end(&mut raw).ok()?;
    // Keep what was known when the tail holds no answer (a long tool result, say).
    let model = model_in(&String::from_utf8_lossy(&raw)).or_else(|| cache.get(transcript).and_then(|(_, m)| m.clone()));
    if cache.len() > 200 {
        cache.clear();
    }
    cache.insert(transcript.to_path_buf(), (len, model.clone()));
    model
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_model_is_the_last_one_that_answered() {
        let t = r#"{"message":{"model":"claude-opus-5-5"}}
{"message":{"model":"<synthetic>"}}
{"message":{"model":"claude-fable-5-2","content":"x"}}
{"message":{"model":"bad model!"}}"#;
        assert_eq!(model_in(t), Some("claude-fable-5-2".to_string()));
        assert_eq!(model_in("no answers yet"), None);
        assert!(model_ok("claude-opus-5-5[1m]") && !model_ok("a b") && !model_ok(""));
    }
}
