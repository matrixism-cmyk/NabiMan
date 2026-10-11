//! Position and seek in tmux's history, without injecting shell keystrokes.
use super::session::tmux;

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub(crate) struct HistoryState {
    pub pane: String,
    pub total: u32,
    pub offset: u32,
    pub rows: u32,
    pub available: bool,
    #[serde(skip)]
    mode: String,
}

pub(crate) fn snapshot(session: &str) -> Option<HistoryState> {
    let out = tmux()
        .args([
            "display-message",
            "-p",
            "-t",
            session,
            "#{pane_id}|#{history_size}|#{scroll_position}|#{pane_height}|#{pane_mode}",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    parse(&String::from_utf8_lossy(&out.stdout))
}

fn parse(text: &str) -> Option<HistoryState> {
    let parts: Vec<_> = text.trim().split('|').collect();
    if parts.len() != 5 || !parts[0].starts_with('%') {
        return None;
    }
    let total = parts[1].parse().ok()?;
    let offset = if parts[2].is_empty() {
        0
    } else {
        parts[2].parse::<u32>().ok()?
    }
    .min(total);
    Some(HistoryState {
        pane: parts[0].into(),
        total,
        offset,
        rows: parts[3].parse().ok()?,
        available: parts[4].is_empty() || parts[4] == "copy-mode",
        mode: parts[4].into(),
    })
}

pub(crate) fn seek(session: &str, offset: u32) {
    let Some(state) = snapshot(session) else {
        return;
    };
    // Do not dismiss choose-tree or another tmux mode while dragging.
    if !state.available {
        return;
    }
    let offset = offset.min(state.total);
    if offset == 0 {
        if state.mode == "copy-mode" {
            let _ = tmux()
                .args(["send-keys", "-t", &state.pane, "-X", "cancel"])
                .output();
        }
        return;
    }
    if state.mode.is_empty() {
        let _ = tmux().args(["copy-mode", "-e", "-t", &state.pane]).output();
    }
    // Reposition relative to the actual copy-mode offset, not the cursor row.
    let Some(current) = snapshot(&state.pane) else {
        return;
    };
    if current.mode != "copy-mode" {
        return;
    }
    let delta = offset.abs_diff(current.offset);
    if delta == 0 {
        return;
    }
    let direction = if offset > current.offset {
        "scroll-up"
    } else {
        "scroll-down"
    };
    let _ = tmux()
        .args([
            "send-keys",
            "-t",
            &state.pane,
            "-X",
            "-N",
            &delta.to_string(),
            direction,
        ])
        .output();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_live_and_copy_positions() {
        let live = parse("%2|240||30|\n").unwrap();
        assert_eq!(live.offset, 0);
        assert!(live.available);
        let copy = parse("%2|240|120|30|copy-mode\n").unwrap();
        assert_eq!(copy.offset, 120);
        assert!(!parse("%2|240||30|tree-mode").unwrap().available);
    }
    #[test]
    fn rejects_invalid_and_clamps_stale_position() {
        assert!(parse("error").is_none());
        assert!(parse("%2|bad|1|30|").is_none());
        assert_eq!(parse("%2|20|21|30|copy-mode").unwrap().offset, 20);
    }
}
