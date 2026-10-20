use super::*;

pub(crate) fn exact_session(name: &str) -> String {
    format!("={name}")
}
pub(super) fn exact_pane(name: &str) -> String {
    format!("={name}:")
}

pub(super) fn classify_tmux_probe(
    output: &std::process::Output,
    name: &str,
) -> std::io::Result<bool> {
    if output.status.success() {
        return Ok(true);
    }
    if output.status.code() == Some(1)
        && [
            format!("can't find session: {name}"),
            format!("can't find session: ={name}"),
        ]
        .iter()
        .any(|expected| String::from_utf8_lossy(&output.stderr).trim() == expected)
    {
        return Ok(false);
    }
    Err(std::io::Error::other(
        "Unable to confirm tmux session state",
    ))
}
pub(crate) fn tmux_session_state(name: &str) -> std::io::Result<bool> {
    let output = tmux()
        .env("LC_ALL", "C")
        .args(["has-session", "-t", &exact_session(name)])
        .output()?;
    classify_tmux_probe(&output, name)
}
pub(crate) fn tmux_session_exists(name: &str) -> bool {
    matches!(tmux_session_state(name), Ok(true))
}
pub(crate) fn close_tmux_session(name: &str) -> std::io::Result<()> {
    let output = tmux()
        .env("LC_ALL", "C")
        .args(["kill-session", "-t", &exact_session(name)])
        .output()?;
    if output.status.success() || matches!(tmux_session_state(name), Ok(false)) {
        Ok(())
    } else {
        Err(std::io::Error::other(
            "Unable to confirm terminal termination; metadata retained",
        ))
    }
}

#[cfg(test)]
#[path = "session_probe_tests.rs"]
mod probe_tests;

/// Options and mouse bindings every NabiMan tmux session runs with.
///
/// The mouse bindings differ from tmux's defaults in one way: a selection is
/// copied *without* leaving copy-mode, so the highlight stays on screen instead
/// of blinking out the moment the button is released. `set-clipboard on` makes
/// tmux hand that copied text to the browser as an OSC 52 sequence.
pub(super) fn tmux_conf_body(scrollback: u32) -> String {
    // window-size latest keeps a watching share viewer from shrinking the pane
    // for whoever is actually working in it.
    //
    // Keeping the highlight means staying in copy-mode, where keystrokes go to
    // copy-mode instead of the shell — so a click of either button leaves it
    // again, which is also what makes a right-click paste land.
    //
    // `copy-pipe-no-clear "tmux load-buffer -w -"` is what actually reaches the
    // browser: it keeps the highlight on screen after the button comes up, and
    // -w makes tmux hand the text over as an OSC 52 sequence, which the
    // frontend turns into a clipboard write. set-clipboard alone does not do
    // it for tmux's own copy commands.
    let copy = r#"send -X copy-pipe-no-clear "tmux load-buffer -w -""#;
    format!(
        "set -g history-limit {scrollback}\n\
         set -g mouse on\n\
         set -g set-clipboard on\n\
         set -g window-size latest\n\
         set -as terminal-features ',xterm*:clipboard'\n\
         bind -T copy-mode    MouseDragEnd1Pane {copy}\n\
         bind -T copy-mode-vi MouseDragEnd1Pane {copy}\n\
         bind -T copy-mode    DoubleClick1Pane send -X select-word \\; {copy}\n\
         bind -T copy-mode-vi DoubleClick1Pane send -X select-word \\; {copy}\n\
         bind -T copy-mode    TripleClick1Pane send -X select-line \\; {copy}\n\
         bind -T copy-mode-vi TripleClick1Pane send -X select-line \\; {copy}\n\
         bind -T root DoubleClick1Pane select-pane \\; copy-mode -H \\; send -X select-word \\; {copy}\n\
         bind -T root TripleClick1Pane select-pane \\; copy-mode -H \\; send -X select-line \\; {copy}\n\
         bind -T copy-mode    MouseDown1Pane send -X cancel\n\
         bind -T copy-mode-vi MouseDown1Pane send -X cancel\n\
         bind -T copy-mode    MouseDown3Pane send -X cancel\n\
         bind -T copy-mode-vi MouseDown3Pane send -X cancel\n",
        scrollback = scrollback,
        copy = copy,
    )
}

/// Seed options for a tmux server this process may be about to start.
pub(super) fn write_tmux_conf(scrollback: u32) -> Option<String> {
    let path = format!("{}/tmux.conf", data_dir());
    std::fs::create_dir_all(data_dir()).ok()?;
    std::fs::write(&path, tmux_conf_body(scrollback)).ok()?;
    Some(path)
}

pub(crate) fn create_tmux_session(
    name: &str,
    command: Option<&str>,
    scrollback: u32,
    size: (u16, u16),
) -> Result<(), String> {
    // history-limit only applies to panes created when they are made, so the
    // value has to be in place *before* the session is spawned. Two paths cover
    // both cases: -f seeds the options when this command starts the tmux server
    // (the first session after a reboot), and set-option -g covers every later
    // session on an already running server.
    let conf = write_tmux_conf(scrollback);
    // -f only applies when this command starts the server, so a server that is
    // already up gets the same file sourced explicitly.
    if let Some(ref path) = conf {
        let _ = tmux().args(["source-file", path]).output();
    }
    let _ = tmux()
        .args(["set-option", "-g", "history-limit", &scrollback.to_string()])
        .output();

    // Create the pane at the size the browser already has: attaching a smaller
    // client would otherwise shrink the pane and push the first lines (an ssh
    // error, for instance) straight out of view.
    let cols = size.0.to_string();
    let rows = size.1.to_string();
    let mut args: Vec<&str> = Vec::new();
    if let Some(ref path) = conf {
        args.push("-f");
        args.push(path);
    }
    args.extend(["new-session", "-d", "-s", name, "-x", &cols, "-y", &rows]);
    if let Some(cmd) = command {
        args.push(cmd);
    }
    let out = tmux()
        .args(&args)
        .output()
        .map_err(|e| format!("tmux: {}", e))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    // Enable mouse so the wheel scrolls tmux's own history (copy-mode) like a
    // native terminal — otherwise, in tmux's alternate screen, xterm.js turns
    // the wheel into arrow keys, which bash treats as command-history nav.
    let _ = tmux()
        .args(["set-option", "-t", &exact_pane(name), "mouse", "on"])
        .output();
    let _ = tmux()
        .args([
            "set-option",
            "-t",
            &exact_pane(name),
            "history-limit",
            &scrollback.to_string(),
        ])
        .output();
    // Detaching a browser must never tear the session down.
    let _ = tmux()
        .args([
            "set-option",
            "-t",
            &exact_pane(name),
            "destroy-unattached",
            "off",
        ])
        .output();
    Ok(())
}

/// `tmux` with the inherited session variables cleared. Without this, a server
/// started from inside a tmux pane would talk to that pane's socket instead of
/// its own.
pub(crate) fn tmux() -> std::process::Command {
    let mut cmd = std::process::Command::new("tmux");
    cmd.env_remove("TMUX").env_remove("TMUX_PANE");
    cmd
}

pub(super) fn tmux_size(args: &[&str]) -> Option<(u16, u16)> {
    let out = tmux().args(args).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut parts = text.split_whitespace();
    let cols = parts.next()?.parse().ok()?;
    let rows = parts.next()?.parse().ok()?;
    Some((cols, rows))
}

/// The terminal size a share viewer should open with: whatever the person
/// already attached is using, status line included. Matching the pane instead
/// would cost a row to the status bar and quietly shrink the session.
pub(crate) fn client_size(name: &str) -> Option<(u16, u16)> {
    tmux_size(&[
        "list-clients",
        "-t",
        &exact_session(name),
        "-F",
        "#{client_width} #{client_height}",
    ])
    .or_else(|| {
        // Nobody is watching: the pane plus the status line it draws under it.
        tmux_size(&[
            "display-message",
            "-p",
            "-t",
            &exact_pane(name),
            "#{pane_width} #{pane_height}",
        ])
        .map(|(c, r)| (c, r + 1))
    })
}

/// The last few lines on screen, as plain text — enough for a thumbnail in the
/// session list without shipping a whole screen of escape codes.
pub(crate) fn pane_preview(name: &str, lines: usize) -> String {
    let out = match tmux()
        .args(["capture-pane", "-p", "-J", "-t", &exact_pane(name)])
        .output()
    {
        Ok(o) if o.status.success() => o.stdout,
        _ => return String::new(),
    };
    let text = String::from_utf8_lossy(&out);
    let kept: Vec<&str> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .rev()
        .take(lines)
        .collect();
    kept.into_iter()
        .rev()
        .map(|l| l.chars().take(200).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Show the operator's name for a session in tmux's own status line too.
pub(crate) fn rename_tmux_window(name: &str, title: &str) {
    let _ = tmux()
        .args(["rename-window", "-t", &exact_pane(name), title])
        .output();
}

/// Hand text to a pane through tmux's own paste buffer.
///
/// Writing the bytes straight into the pseudo-terminal works until the pane is
/// in copy-mode, where they are swallowed, and it loses the bracketed-paste
/// markers that let an editor tell a paste from typing. tmux does both
/// properly, so the paste goes through tmux.
pub(crate) fn paste_into_pane(name: &str, text: &str) {
    use std::io::Write;

    // A selection left the pane in copy-mode; leave it before pasting.
    if tmux_size(&[
        "display-message",
        "-p",
        "-t",
        &exact_pane(name),
        "#{pane_in_mode} 0",
    ])
    .map(|(mode, _)| mode == 1)
    .unwrap_or(false)
    {
        let _ = tmux()
            .args(["send-keys", "-X", "-t", &exact_pane(name), "cancel"])
            .output();
    }

    let buffer = format!("nabiman-paste-{}", name);
    let mut child = match tmux()
        .args(["load-buffer", "-b", &buffer, "-"])
        .stdin(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return,
    };
    if let Some(stdin) = child.stdin.as_mut() {
        let _ = stdin.write_all(text.as_bytes());
    }
    let _ = child.wait();
    let _ = tmux()
        .args(["paste-buffer", "-d", "-b", &buffer, "-t", &exact_pane(name)])
        .output();
}

/// Read back the pane history (ANSI colours included) so a reopened window can
/// show what scrolled past while it was closed.
pub(crate) fn capture_scrollback(name: &str, lines: u32) -> Result<String, String> {
    let start = format!("-{}", lines.min(MAX_SCROLLBACK));
    let out = tmux()
        .args([
            "capture-pane",
            "-p",
            "-e",
            "-J",
            "-S",
            &start,
            "-t",
            &exact_pane(name),
        ])
        .output()
        .map_err(|e| format!("tmux: {}", e))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

#[cfg(test)]
#[path = "session_exact_target_tests.rs"]
mod exact_target_tests;
