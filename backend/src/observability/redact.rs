//! Keeps secrets out of log lines: a field whose name says it holds one is
//! replaced, and free text is scanned for the usual shapes — `password=…`,
//! `"token":"…"`, `Authorization: Bearer …`, `nbm_…` API keys, JWTs and the
//! password in `scheme://user:pass@host`. It errs on the side of hiding.

const HIDDEN: &str = "[redacted]";

/// Words that, in a name, mean the value is a secret.
const SECRET_PARTS: &[&str] = &[
    "password", "passwd", "passphrase", "secret", "token", "apikey", "api_key", "api-key",
    "authorization", "cookie", "credential", "private_key", "privatekey", "session_key",
];
/// Short names that are secrets only on their own.
const SECRET_NAMES: &[&str] = &["pass", "pwd", "key", "auth", "sig", "signature", "ticket", "otp", "totp", "sid"];

pub(crate) fn sensitive_key(name: &str) -> bool {
    let name = name.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-').to_ascii_lowercase();
    !name.is_empty() && (SECRET_PARTS.iter().any(|p| name.contains(p)) || SECRET_NAMES.contains(&name.as_str()))
}

/// A field's value as it may be written.
pub(crate) fn field(name: &str, value: &str) -> String {
    if sensitive_key(name) { HIDDEN.into() } else { text(value) }
}

fn core(word: &str) -> &str {
    word.trim_matches(|c: char| matches!(c, '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | ',' | ';' | '.'))
}

fn looks_like_token(word: &str) -> bool {
    let w = core(word);
    (w.starts_with("nbm_") && w.len() > 8)
        || (w.starts_with("eyJ") && w.matches('.').count() == 2 && w.len() > 20)
}

/// `scheme://user:pass@host` without the password.
fn without_userinfo_password(segment: &str) -> Option<String> {
    let start = segment.find("://")? + 3;
    let rest = &segment[start..];
    let at = rest.find('@')?;
    let userinfo = &rest[..at];
    if userinfo.contains('/') {
        return None;
    }
    let colon = userinfo.find(':')?;
    Some(format!("{}{}:{HIDDEN}{}", &segment[..start], &userinfo[..colon], &rest[at..]))
}

/// One `key=value` / `key:value` piece, or a bare token.
fn segment(seg: &str) -> String {
    if let Some(clean) = without_userinfo_password(seg) {
        return clean;
    }
    if let Some(i) = seg.find(['=', ':']) {
        let (key, value) = (&seg[..i], &seg[i + 1..]);
        if sensitive_key(key) && !value.trim_matches(|c| c == '"' || c == '\'').is_empty() {
            return format!("{}{HIDDEN}", &seg[..=i]);
        }
    }
    if looks_like_token(seg) { HIDDEN.into() } else { seg.to_string() }
}

/// One whitespace-free word: its `&`, `?`, `,` and `;` pieces one by one.
fn word(w: &str) -> String {
    let mut out = String::with_capacity(w.len());
    let mut piece = String::new();
    for c in w.chars() {
        if matches!(c, '&' | '?' | ',' | ';') {
            out.push_str(&segment(&piece));
            out.push(c);
            piece.clear();
        } else {
            piece.push(c);
        }
    }
    out.push_str(&segment(&piece));
    out
}

/// Free text with the secrets in it replaced.
pub(crate) fn text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut hide_next = false;
    let mut rest = s;
    while !rest.is_empty() {
        let space = rest.len() - rest.trim_start().len();
        out.push_str(&rest[..space]);
        rest = &rest[space..];
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let w = &rest[..end];
        rest = &rest[end..];
        if w.is_empty() {
            continue;
        }
        let lower = core(w).to_ascii_lowercase();
        // `Authorization: Token x` hides x; a plain "token" in prose hides nothing.
        let scheme = matches!(lower.as_str(), "bearer" | "basic");
        if hide_next && !(scheme || matches!(lower.as_str(), "token" | "digest")) {
            out.push_str(HIDDEN);
            hide_next = false;
            continue;
        }
        let key = core(w.trim_end_matches([':', '=']));
        if scheme || hide_next || ((w.ends_with(':') || w.ends_with('=')) && sensitive_key(key)) {
            out.push_str(w);
            hide_next = true;
            continue;
        }
        out.push_str(&word(w));
    }
    out
}
