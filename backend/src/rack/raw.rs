//! The layout file as it is written, before anything in it is checked
//! (`layout::parse` does that). Fields the page has no use for are not named
//! here and so are dropped on reading.

use std::collections::BTreeMap;

use serde::Deserialize;

#[derive(Deserialize)]
pub(super) struct RawLayout {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub notes: Option<String>,
    pub racks: Vec<RawRack>,
    #[serde(default)]
    pub power: Option<super::ordered::Ordered>,
}

#[derive(Deserialize)]
pub(super) struct RawRack {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub role: Option<String>,
    pub units: u32,
    /// Somebody else's rack: its devices are theirs unless one says otherwise.
    #[serde(default)]
    pub external: Option<bool>,
    #[serde(default)]
    pub devices: Vec<RawDevice>,
    #[serde(default)]
    pub node_info: Option<BTreeMap<String, RawNode>>,
}

#[derive(Deserialize)]
pub(super) struct RawDevice {
    pub id: String,
    pub label: String,
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    #[serde(default)]
    pub u: Option<u32>,
    #[serde(default)]
    pub h: Option<u32>,
    #[serde(default)]
    pub u_confirmed: bool,
    #[serde(default)]
    pub node: Option<String>,
    #[serde(default)]
    pub node_candidates: Option<Vec<String>>,
    #[serde(default)]
    pub ip: Option<String>,
    #[serde(default)]
    pub bmc: Option<String>,
    #[serde(default)]
    pub gpu: Option<String>,
    #[serde(default)]
    pub cpu: Option<u32>,
    #[serde(default)]
    pub arch: Option<String>,
    #[serde(default)]
    pub desc: Option<String>,
    #[serde(default)]
    pub order_hint: Option<i32>,
    /// How to see whether it answers: `ping`, `tcp:<port>` or `none`.
    #[serde(default)]
    pub check: Option<String>,
    /// Not ours (true) or ours (false), whatever its rack says.
    #[serde(default)]
    pub external: Option<bool>,
}

#[derive(Deserialize)]
pub(super) struct RawNode {
    #[serde(default)]
    pub ip: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub cpu: Option<u32>,
    #[serde(default)]
    pub mem_gi: Option<u32>,
    #[serde(default)]
    pub gpu: Option<String>,
}
