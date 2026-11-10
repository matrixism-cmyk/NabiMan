//! The routes the real App registers, read back from actix's own resource
//! map. actix has no public way to list them, but `ResourceMap` is `Debug`,
//! so a request answered inside the App prints the whole tree and this reads
//! it: every leaf's full pattern is the patterns of its scopes joined.

use actix_web::{test, web, App, HttpRequest};

#[derive(Debug)]
enum V {
    Str(String),
    Atom,
    Struct(Vec<(String, V)>),
    Tuple(String, Vec<V>),
    List(Vec<V>),
    Map,
}

struct P<'a> {
    s: &'a [u8],
    i: usize,
}

impl P<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn peek(&mut self) -> u8 {
        self.ws();
        self.s.get(self.i).copied().unwrap_or(0)
    }
    fn eat(&mut self, c: u8) -> bool {
        (self.peek() == c).then(|| self.i += 1).is_some()
    }
    fn string(&mut self) -> String {
        let start = self.i;
        self.i += 1;
        while self.s[self.i] != b'"' {
            self.i += if self.s[self.i] == b'\\' { 2 } else { 1 };
        }
        self.i += 1;
        // A Debug string is a Rust string literal; patterns hold no escapes that matter.
        String::from_utf8_lossy(&self.s[start + 1..self.i - 1]).replace("\\\"", "\"").replace("\\\\", "\\")
    }
    fn items(&mut self, close: u8) -> Vec<V> {
        let mut out = vec![];
        while !self.eat(close) {
            out.push(self.value());
            if self.eat(b':') {
                let _ = self.value(); // a map's value; keys and values are not needed
            }
            self.eat(b',');
        }
        out
    }
    fn value(&mut self) -> V {
        match self.peek() {
            b'"' => V::Str(self.string()),
            b'\'' => {
                self.i += 1;
                while self.s[self.i] != b'\'' {
                    self.i += if self.s[self.i] == b'\\' { 2 } else { 1 };
                }
                self.i += 1;
                V::Atom
            }
            b'[' => {
                self.i += 1;
                V::List(self.items(b']'))
            }
            b'{' => {
                self.i += 1;
                self.items(b'}');
                V::Map
            }
            b'(' => {
                self.i += 1;
                V::Tuple(String::new(), self.items(b')'))
            }
            _ => {
                let start = self.i;
                while self.i < self.s.len() && !b",:)]}({[ \"".contains(&self.s[self.i]) {
                    self.i += 1;
                }
                let name = String::from_utf8_lossy(&self.s[start..self.i]).into_owned();
                if self.eat(b'{') {
                    let mut fields = vec![];
                    while !self.eat(b'}') {
                        self.ws();
                        let fs = self.i;
                        while self.s[self.i] != b':' {
                            self.i += 1;
                        }
                        let field = String::from_utf8_lossy(&self.s[fs..self.i]).trim().to_string();
                        self.i += 1;
                        fields.push((field, self.value()));
                        self.eat(b',');
                    }
                    V::Struct(fields)
                } else if self.s.get(self.i) == Some(&b'(') {
                    self.i += 1;
                    V::Tuple(name, self.items(b')'))
                } else {
                    V::Atom
                }
            }
        }
    }
}

fn field<'a>(v: &'a V, name: &str) -> Option<&'a V> {
    match v {
        V::Struct(fields) => fields.iter().find(|(f, _)| f == name).map(|(_, v)| v),
        _ => None,
    }
}

fn patterns(def: &V) -> Vec<String> {
    match field(def, "patterns") {
        Some(V::Tuple(kind, items)) if kind == "Single" => items.iter().filter_map(|v| match v { V::Str(s) => Some(s.clone()), _ => None }).collect(),
        Some(V::Tuple(kind, items)) if kind == "List" => match items.first() {
            Some(V::List(list)) => list.iter().filter_map(|v| match v { V::Str(s) => Some(s.clone()), _ => None }).collect(),
            _ => vec![],
        },
        other => panic!("unexpected resource patterns: {other:?}"),
    }
}

fn walk(map: &V, prefix: &str, out: &mut Vec<String>) {
    let def = field(map, "pattern").expect("a ResourceMap has a pattern");
    for own in patterns(def) {
        let full = format!("{prefix}{own}");
        match field(map, "nodes") {
            Some(V::Tuple(some, items)) if some == "Some" => {
                if let Some(V::List(children)) = items.first() {
                    for child in children {
                        walk(child, &full, out);
                    }
                }
            }
            _ => out.push(full),
        }
    }
}

/// Parses `format!("{:?}", resource_map)` into full route patterns.
pub(crate) fn leaves(debug: &str) -> Vec<String> {
    let tree = P { s: debug.as_bytes(), i: 0 }.value();
    let mut out = vec![];
    walk(&tree, "", &mut out);
    out
}

/// Every route pattern of the App `main` serves (the API and the probes
/// outside it; not the static files), sorted, once each.
pub(crate) async fn api_patterns() -> Vec<String> {
    let app = test::init_service(App::new().configure(crate::app_routes::configure).route(
        "/__route_inventory",
        web::get().to(|req: HttpRequest| async move { format!("{:?}", req.resource_map()) }),
    ))
    .await;
    let body = test::call_and_read_body(&app, test::TestRequest::get().uri("/__route_inventory").to_request()).await;
    let mut all: Vec<String> = leaves(std::str::from_utf8(&body).unwrap())
        .into_iter()
        .filter(|p| p != "/__route_inventory")
        .collect();
    all.sort();
    all.dedup();
    all
}
