/// Cleans raw AXGATE shell output before handing it to the parser.
///
/// Real AXGATE CLI sessions (via PTY) include artifacts that trip the naive
/// line-oriented parser:
///  - ANSI / VT100 escape sequences (cursor moves, color codes)
///  - Echoed command lines (we type `show running-config\r`, AXGATE echoes it back)
///  - CLI prompts intermixed with output (`HOSTNAME#`, `HOSTNAME(config)#`)
///  - Pagination markers if `terminal length 0` was not honored
///  - `\r\n` vs `\n` inconsistency
///
/// This preprocessor produces a single `\n`-joined body with echoes and
/// prompts stripped, leaving only lines that look like config content.
pub fn clean(raw: &str) -> String {
    let without_ansi = strip_ansi(raw);
    let normalized = normalize_newlines(&without_ansi);
    let filtered = filter_lines(&normalized);
    filtered
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            match chars.next() {
                Some('[') => {
                    // CSI sequence: read until final byte 0x40..0x7E
                    while let Some(&n) = chars.peek() {
                        chars.next();
                        if ('\x40'..='\x7e').contains(&n) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    // OSC: read until BEL or ST (ESC \)
                    while let Some(n) = chars.next() {
                        if n == '\x07' {
                            break;
                        }
                        if n == '\x1b' {
                            let _ = chars.next();
                            break;
                        }
                    }
                }
                Some(_) | None => {}
            }
        } else if c == '\r' {
            // skip carriage returns entirely (will re-normalize below)
        } else if c == '\x08' || c == '\x07' {
            // backspace / bell
        } else {
            out.push(c);
        }
    }
    out
}

fn normalize_newlines(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

fn filter_lines(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for line in s.lines() {
        if is_noise(line) {
            continue;
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

fn is_noise(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() {
        return false; // keep empty lines for block boundaries
    }
    if t.starts_with("Password") || t.starts_with("Username") {
        return true;
    }
    if t == "--More--" || t.starts_with("--More--") {
        return true;
    }
    // Echoed commands we sent
    if t == "terminal length 0" || t == "show running-config" || t == "exit" {
        return true;
    }
    if is_prompt(t) {
        return true;
    }
    false
}

fn is_prompt(t: &str) -> bool {
    // e.g. "HOSTNAME#", "HOSTNAME(config)#", "AXGATE>", "hostname#"
    if let Some(last) = t.chars().last() {
        if !matches!(last, '#' | '>') {
            return false;
        }
    } else {
        return false;
    }
    let body = &t[..t.len() - 1];
    let body = body.trim_end_matches(')');
    let body = body.trim_end_matches(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '(');
    body.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_ansi_csi() {
        let s = "\x1b[32mhello\x1b[0m world";
        assert_eq!(strip_ansi(s), "hello world");
    }

    #[test]
    fn normalizes_crlf() {
        let s = "a\r\nb\rc\nd";
        assert_eq!(normalize_newlines(s), "a\nb\nc\nd");
    }

    #[test]
    fn prompt_detected() {
        assert!(is_prompt("HOSTNAME#"));
        assert!(is_prompt("HOSTNAME(config)#"));
        assert!(is_prompt("axgate>"));
        assert!(!is_prompt("not a prompt"));
        assert!(!is_prompt("something# else"));
    }

    #[test]
    fn filter_strips_echoes_and_prompts() {
        let raw = "Password: \r\nAXGATE#\r\nterminal length 0\r\nAXGATE#\r\nshow running-config\r\nip nat policy from untrust to trust 10\r\n  dnat destination 121.147.13.233/32\r\nAXGATE#\r\nexit\r\n";
        let cleaned = clean(raw);
        assert!(cleaned.contains("ip nat policy from untrust to trust 10"));
        assert!(cleaned.contains("dnat destination 121.147.13.233/32"));
        assert!(!cleaned.contains("show running-config"));
        assert!(!cleaned.contains("AXGATE#"));
        assert!(!cleaned.contains("Password"));
    }

    #[test]
    fn handles_lockout_message_gracefully() {
        // Lockout banner should pass through (parser produces 0 rules, which is correct).
        let raw = "\r\n% You are blocked for 600 seconds!\r\nUsername: ";
        let cleaned = clean(raw);
        assert!(cleaned.contains("blocked for 600 seconds"));
    }

    #[test]
    fn more_paginator_removed() {
        let raw = "line1\r\n--More--\r\nline2\r\n";
        let cleaned = clean(raw);
        assert!(cleaned.contains("line1"));
        assert!(cleaned.contains("line2"));
        assert!(!cleaned.contains("--More--"));
    }
}
