//! Reading the terminal WebSocket's query string.

pub(crate) fn parse_query_param(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|p| {
        let mut kv = p.splitn(2, '=');
        if kv.next() == Some(key) {
            kv.next().map(urlencoding_decode)
        } else {
            None
        }
    })
}
fn urlencoding_decode(s: &str) -> String {
    let mut r = String::with_capacity(s.len());
    let mut c = s.bytes();
    while let Some(b) = c.next() {
        if b == b'%' {
            let h = c.next().unwrap_or(b'0');
            let l = c.next().unwrap_or(b'0');
            r.push((hv(h) * 16 + hv(l)) as char);
        } else if b == b'+' {
            r.push(' ');
        } else {
            r.push(b as char);
        }
    }
    r
}
fn hv(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        b'A'..=b'F' => b - b'A' + 10,
        _ => 0,
    }
}
pub(super) fn validate_ssh_input(s: &str) -> bool {
    // Spliced into the pane's ssh line: a leading '-' would be an option.
    crate::cli_args::not_option(s)
        && s.len() < 256
        && s.chars()
            .all(|c| c.is_alphanumeric() || ".-_@:".contains(c))
}
