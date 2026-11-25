//! A JSON object read with its entries in the order they were written.
//!
//! The layout's `power` notes are shown as a list, and whoever writes the file
//! puts them in the order they should be read (mains, then UPS, then a note).
//! A plain map would hand them back sorted by key.

use serde::de::{Deserializer, MapAccess, Visitor};
use serde::Deserialize;

/// An object's entries as written. Entries past `MAX` are counted, not kept.
#[derive(Debug, Default, PartialEq)]
pub(super) struct Ordered {
    pub entries: Vec<(String, serde_json::Value)>,
    pub total: usize,
}

/// Entries kept at most (the reader's own limit is checked against `total`).
const MAX: usize = 256;

impl<'de> Deserialize<'de> for Ordered {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Entries;
        impl<'de> Visitor<'de> for Entries {
            type Value = Ordered;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Ordered, A::Error> {
                let mut out = Ordered::default();
                while let Some((key, value)) = map.next_entry::<String, serde_json::Value>()? {
                    out.total += 1;
                    if out.entries.len() < MAX {
                        out.entries.push((key, value));
                    }
                }
                Ok(out)
            }
        }
        deserializer.deserialize_map(Entries)
    }
}
