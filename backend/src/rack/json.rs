//! A JSON document kept as it was written: objects with their entries in
//! file order, numbers as their own text. `serde_json::Value` here sorts
//! keys and re-renders numbers; the layout file is other people's too, so
//! a save must change only what was moved (see `moves`).
//!
//! Written back in the form the file is kept in: UTF-8 with non-ASCII as
//! itself, two spaces of indent, `": "` after a key, a trailing newline.

/// Deeper nesting than this is refused rather than followed.
const MAX_DEPTH: usize = 64;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Json {
    Null,
    Bool(bool),
    /// The number's text, exactly as written.
    Number(String),
    String(String),
    Array(Vec<Json>),
    /// Entries in the order written (a repeated key stays repeated).
    Object(Vec<(String, Json)>),
}

impl Json {
    pub(super) fn text(value: &str) -> Json {
        Json::String(value.to_string())
    }

    pub(super) fn number(value: u32) -> Json {
        Json::Number(value.to_string())
    }

    pub(super) fn as_str(&self) -> Option<&str> {
        match self {
            Json::String(s) => Some(s),
            _ => None,
        }
    }

    pub(super) fn as_array_mut(&mut self) -> Option<&mut Vec<Json>> {
        match self {
            Json::Array(items) => Some(items),
            _ => None,
        }
    }

    pub(super) fn entries(&self) -> Option<&Vec<(String, Json)>> {
        match self {
            Json::Object(entries) => Some(entries),
            _ => None,
        }
    }

    pub(super) fn entries_mut(&mut self) -> Option<&mut Vec<(String, Json)>> {
        match self {
            Json::Object(entries) => Some(entries),
            _ => None,
        }
    }

    /// An object's first entry of this name.
    pub(super) fn get(&self, key: &str) -> Option<&Json> {
        self.entries()?.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub(super) fn get_mut(&mut self, key: &str) -> Option<&mut Json> {
        self.entries_mut()?.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// Give an object's entry a new value where it stands; a new entry goes
    /// right after the first of `after` the object has, else at the end.
    pub(super) fn set(&mut self, key: &str, value: Json, after: &[&str]) {
        let Some(entries) = self.entries_mut() else { return };
        if let Some(entry) = entries.iter_mut().find(|(k, _)| k == key) {
            entry.1 = value;
            return;
        }
        let at = after.iter().find_map(|a| entries.iter().position(|(k, _)| k == a)).map_or(entries.len(), |i| i + 1);
        entries.insert(at, (key.to_string(), value));
    }

    pub(super) fn parse(bytes: &[u8]) -> Result<Json, String> {
        let text = std::str::from_utf8(bytes).map_err(|e| format!("not UTF-8: {e}"))?;
        let mut reader = Reader { text, bytes: text.as_bytes(), at: 0 };
        reader.space();
        let value = reader.value(0)?;
        reader.space();
        if reader.at != reader.bytes.len() {
            return Err(reader.fault("text after the document"));
        }
        Ok(value)
    }

    /// The document's text, ending in a newline.
    pub(super) fn write(&self) -> String {
        let mut out = String::new();
        self.write_into(&mut out, 0);
        out.push('\n');
        out
    }

    fn write_into(&self, out: &mut String, depth: usize) {
        let indent = |out: &mut String, depth: usize| {
            out.push('\n');
            out.extend(std::iter::repeat("  ").take(depth));
        };
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Json::Number(n) => out.push_str(n),
            Json::String(s) => quote(s, out),
            Json::Array(items) if items.is_empty() => out.push_str("[]"),
            Json::Object(entries) if entries.is_empty() => out.push_str("{}"),
            Json::Array(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    indent(out, depth + 1);
                    item.write_into(out, depth + 1);
                }
                indent(out, depth);
                out.push(']');
            }
            Json::Object(entries) => {
                out.push('{');
                for (i, (key, value)) in entries.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    indent(out, depth + 1);
                    quote(key, out);
                    out.push_str(": ");
                    value.write_into(out, depth + 1);
                }
                indent(out, depth);
                out.push('}');
            }
        }
    }
}

/// A string as JSON: only what must be escaped is.
fn quote(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

struct Reader<'a> {
    text: &'a str,
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn fault(&self, what: &str) -> String {
        let line = self.bytes[..self.at.min(self.bytes.len())].iter().filter(|b| **b == b'\n').count() + 1;
        format!("{what} (line {line})")
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    fn space(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.at += 1;
        }
    }

    fn word(&mut self, word: &str, value: Json) -> Result<Json, String> {
        if self.bytes[self.at..].starts_with(word.as_bytes()) {
            self.at += word.len();
            Ok(value)
        } else {
            Err(self.fault("not a JSON value"))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Json, String> {
        if depth > MAX_DEPTH {
            return Err(self.fault("nested too deeply"));
        }
        match self.peek() {
            Some(b'n') => self.word("null", Json::Null),
            Some(b't') => self.word("true", Json::Bool(true)),
            Some(b'f') => self.word("false", Json::Bool(false)),
            Some(b'"') => self.string().map(Json::String),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(b'[') => {
                self.at += 1;
                let mut items = Vec::new();
                self.space();
                if self.peek() == Some(b']') {
                    self.at += 1;
                    return Ok(Json::Array(items));
                }
                loop {
                    self.space();
                    items.push(self.value(depth + 1)?);
                    self.space();
                    match self.peek() {
                        Some(b',') => self.at += 1,
                        Some(b']') => {
                            self.at += 1;
                            return Ok(Json::Array(items));
                        }
                        _ => return Err(self.fault("a list needs , or ]")),
                    }
                }
            }
            Some(b'{') => {
                self.at += 1;
                let mut entries = Vec::new();
                self.space();
                if self.peek() == Some(b'}') {
                    self.at += 1;
                    return Ok(Json::Object(entries));
                }
                loop {
                    self.space();
                    if self.peek() != Some(b'"') {
                        return Err(self.fault("an object's key is a string"));
                    }
                    let key = self.string()?;
                    self.space();
                    if self.peek() != Some(b':') {
                        return Err(self.fault("a key needs :"));
                    }
                    self.at += 1;
                    self.space();
                    entries.push((key, self.value(depth + 1)?));
                    self.space();
                    match self.peek() {
                        Some(b',') => self.at += 1,
                        Some(b'}') => {
                            self.at += 1;
                            return Ok(Json::Object(entries));
                        }
                        _ => return Err(self.fault("an object needs , or }")),
                    }
                }
            }
            _ => Err(self.fault("not a JSON value")),
        }
    }

    fn digits(&mut self) -> usize {
        let from = self.at;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.at += 1;
        }
        self.at - from
    }

    /// A number by JSON's grammar; its text is kept, not its value.
    fn number(&mut self) -> Result<Json, String> {
        let from = self.at;
        if self.peek() == Some(b'-') {
            self.at += 1;
        }
        let leading_zero = self.peek() == Some(b'0');
        let whole = self.digits();
        if whole == 0 || (leading_zero && whole > 1) {
            return Err(self.fault("not a number"));
        }
        if self.peek() == Some(b'.') {
            self.at += 1;
            if self.digits() == 0 {
                return Err(self.fault("not a number"));
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.at += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.at += 1;
            }
            if self.digits() == 0 {
                return Err(self.fault("not a number"));
            }
        }
        Ok(Json::Number(self.text[from..self.at].to_string()))
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let digits = self.text.get(self.at..self.at + 4).filter(|d| d.bytes().all(|b| b.is_ascii_hexdigit()));
        let Some(digits) = digits else { return Err(self.fault("a \\u escape needs four hex digits")) };
        self.at += 4;
        Ok(u32::from_str_radix(digits, 16).unwrap_or(0))
    }

    fn string(&mut self) -> Result<String, String> {
        self.at += 1;
        let mut out = String::new();
        loop {
            let from = self.at;
            while !matches!(self.peek(), None | Some(b'"' | b'\\' | 0..=0x1f)) {
                self.at += 1;
            }
            // Stops only on ASCII, so both ends are character boundaries.
            out.push_str(&self.text[from..self.at]);
            match self.peek() {
                Some(b'"') => {
                    self.at += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.at += 1;
                    let escape = self.peek();
                    self.at += 1;
                    match escape {
                        Some(b'"') => out.push('"'),
                        Some(b'\\') => out.push('\\'),
                        Some(b'/') => out.push('/'),
                        Some(b'n') => out.push('\n'),
                        Some(b'r') => out.push('\r'),
                        Some(b't') => out.push('\t'),
                        Some(b'b') => out.push('\u{8}'),
                        Some(b'f') => out.push('\u{c}'),
                        Some(b'u') => out.push(self.escaped()?),
                        _ => return Err(self.fault("an unknown \\ escape")),
                    }
                }
                Some(_) => return Err(self.fault("a control character inside a string")),
                None => return Err(self.fault("a string that does not end")),
            }
        }
    }

    /// The character of a `\uXXXX` escape (two of them for one outside the basic plane).
    fn escaped(&mut self) -> Result<char, String> {
        let first = self.hex4()?;
        let code = if (0xD800..0xDC00).contains(&first) {
            if !self.bytes[self.at..].starts_with(b"\\u") {
                return Err(self.fault("half of a surrogate pair"));
            }
            self.at += 2;
            let second = self.hex4()?;
            if !(0xDC00..0xE000).contains(&second) {
                return Err(self.fault("half of a surrogate pair"));
            }
            0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00)
        } else {
            first
        };
        char::from_u32(code).ok_or_else(|| self.fault("half of a surrogate pair"))
    }
}
