//! The rack layout file: read, checked against fixed limits, and cut down to
//! what the page draws. The file is written by people (and other tools) and
//! changes while NabiMan runs, so every fault in it is a sentence that says
//! where, never a panic. Fields the page does not need (`status_sources`
//! among them: queries are fixed in `prometheus`) are not passed on.

use std::collections::{BTreeMap, HashSet};
use std::net::Ipv4Addr;

use serde::{Deserialize, Serialize};

pub(crate) const MAX_FILE_BYTES: u64 = 1024 * 1024;
pub(crate) const MAX_RACKS: usize = 16;
pub(crate) const MAX_DEVICES: usize = 400;
pub(crate) const MAX_UNITS: u32 = 60;
pub(crate) const MAX_CANDIDATES: usize = 4;
const MAX_NODES: usize = 64;
const MAX_SOURCES: usize = 50;
const MAX_POWER: usize = 20;
/// The device kinds the page has a look for; any other becomes `other`.
const TYPES: &[&str] = &["server", "switch", "firewall", "5g", "storage", "external"];

#[derive(Deserialize)]
struct RawLayout {
    #[serde(default)]
    version: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    sources: Vec<String>,
    #[serde(default)]
    notes: Option<String>,
    racks: Vec<RawRack>,
    #[serde(default)]
    power: Option<BTreeMap<String, serde_json::Value>>,
}

#[derive(Deserialize)]
struct RawRack {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    role: Option<String>,
    units: u32,
    #[serde(default)]
    devices: Vec<RawDevice>,
    #[serde(default)]
    node_info: Option<BTreeMap<String, RawNode>>,
}

#[derive(Deserialize)]
struct RawDevice {
    id: String,
    label: String,
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    u: Option<u32>,
    #[serde(default)]
    h: Option<u32>,
    #[serde(default)]
    u_confirmed: bool,
    #[serde(default)]
    node: Option<String>,
    #[serde(default)]
    node_candidates: Option<Vec<String>>,
    #[serde(default)]
    ip: Option<String>,
    #[serde(default)]
    bmc: Option<String>,
    #[serde(default)]
    gpu: Option<String>,
    #[serde(default)]
    cpu: Option<u32>,
    #[serde(default)]
    arch: Option<String>,
    #[serde(default)]
    desc: Option<String>,
    #[serde(default)]
    order_hint: Option<i32>,
}

#[derive(Deserialize)]
struct RawNode {
    #[serde(default)]
    ip: Option<String>,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    cpu: Option<u32>,
    #[serde(default)]
    mem_gi: Option<u32>,
    #[serde(default)]
    gpu: Option<String>,
}

/// What the browser gets.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub(crate) struct Layout {
    pub version: String,
    pub title: String,
    pub sources: Vec<String>,
    pub notes: Option<String>,
    pub power: Vec<Pair>,
    pub racks: Vec<Rack>,
    /// Every Kubernetes node the file names (on a device, as a candidate, or
    /// in a rack's `node_info`), once each, in the order first met.
    pub nodes: Vec<Node>,
    /// Devices that claim the same slot; drawn all the same, and said so.
    pub overlaps: Vec<Overlap>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub(crate) struct Pair {
    pub label: String,
    pub value: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub(crate) struct Overlap {
    pub rack: String,
    pub first: String,
    pub second: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub(crate) struct Rack {
    pub id: String,
    pub name: String,
    pub role: Option<String>,
    pub units: u32,
    pub devices: Vec<Device>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub(crate) struct Device {
    pub id: String,
    pub label: String,
    #[serde(rename = "type")]
    pub kind: String,
    /// The device's TOP slot (U1 is the bottom of the rack): `u` 27 with `h`
    /// 2 fills U26–U27. `None`: where it sits is not known yet.
    pub u: Option<u32>,
    pub h: u32,
    pub u_confirmed: bool,
    pub node: Option<String>,
    /// The nodes this machine may be, while nobody has checked which.
    pub node_candidates: Vec<String>,
    pub ip: Option<String>,
    pub bmc: Option<String>,
    pub gpu: Option<String>,
    pub cpu: Option<u32>,
    pub arch: Option<String>,
    pub desc: Option<String>,
    pub order_hint: Option<i32>,
}

impl Device {
    /// The nodes whose state this device shows: its own, else its candidates.
    pub(crate) fn nodes(&self) -> Vec<&str> {
        match &self.node {
            Some(node) => vec![node.as_str()],
            None => self.node_candidates.iter().map(String::as_str).collect(),
        }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
pub(crate) struct Node {
    pub name: String,
    pub ip: Option<String>,
    pub role: Option<String>,
    pub model: Option<String>,
    pub cpu: Option<u32>,
    pub mem_gi: Option<u32>,
    pub gpu: Option<String>,
    pub arch: Option<String>,
}

/// A Kubernetes node name as it may appear in a metric label: a DNS name.
pub(crate) fn is_node_name(name: &str) -> bool {
    let edge = |c: Option<char>| c.is_some_and(|c| c.is_ascii_alphanumeric());
    name.len() <= 253
        && edge(name.chars().next())
        && edge(name.chars().last())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
}

fn is_ipv4(text: &str) -> bool {
    text.parse::<Ipv4Addr>().is_ok_and(|ip| ip.to_string() == text)
}

fn text(at: &str, value: &str, max: usize) -> Result<String, String> {
    let value = value.trim();
    if value.chars().count() > max {
        return Err(format!("{at}: longer than {max} characters"));
    }
    if value.chars().any(|c| c.is_control() && c != '\n' && c != '\t') {
        return Err(format!("{at}: contains a control character"));
    }
    Ok(value.to_string())
}

fn optional(at: &str, value: &Option<String>, max: usize) -> Result<Option<String>, String> {
    match value {
        Some(v) => Ok(Some(text(at, v, max)?).filter(|v| !v.is_empty())),
        None => Ok(None),
    }
}

fn id(at: &str, value: &str) -> Result<String, String> {
    let ok = !value.is_empty()
        && value.len() <= 64
        && value.chars().all(|c| c.is_ascii_alphanumeric() || "-_.#".contains(c));
    if ok { Ok(value.to_string()) } else { Err(format!("{at}: an id is 1–64 of letters, digits, - _ . #")) }
}

fn node_name(at: &str, value: &str) -> Result<String, String> {
    if is_node_name(value) { Ok(value.to_string()) } else { Err(format!("{at}: not a node name (letters, digits, - and .)")) }
}

fn ipv4(at: &str, value: &Option<String>) -> Result<Option<String>, String> {
    match value.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        Some(v) if is_ipv4(v) => Ok(Some(v.to_string())),
        Some(_) => Err(format!("{at}: not an IPv4 address")),
        None => Ok(None),
    }
}

/// Parse and check the file's text.
pub(crate) fn parse(bytes: &[u8]) -> Result<Layout, String> {
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(format!("the file is larger than {MAX_FILE_BYTES} bytes"));
    }
    let raw: RawLayout = serde_json::from_slice(bytes).map_err(|e| format!("not a rack layout: {e}"))?;
    if raw.racks.is_empty() || raw.racks.len() > MAX_RACKS {
        return Err(format!("racks: {} given; 1–{MAX_RACKS} allowed", raw.racks.len()));
    }
    let total: usize = raw.racks.iter().map(|r| r.devices.len()).sum();
    if total > MAX_DEVICES {
        return Err(format!("devices: {total} given; at most {MAX_DEVICES} allowed"));
    }
    if raw.sources.len() > MAX_SOURCES {
        return Err(format!("sources: more than {MAX_SOURCES}"));
    }
    let sources = raw.sources.iter().enumerate().map(|(i, s)| text(&format!("sources[{i}]"), s, 500)).collect::<Result<Vec<_>, _>>()?;
    let power = power(raw.power.as_ref())?;

    let mut nodes: Vec<Node> = Vec::new();
    let mut seen_ids = HashSet::new();
    let mut seen_racks = HashSet::new();
    let mut racks = Vec::new();
    let mut overlaps = Vec::new();
    for (ri, rack) in raw.racks.iter().enumerate() {
        let at = format!("racks[{ri}]");
        let rack_id = id(&format!("{at}.id"), &rack.id)?;
        if !seen_racks.insert(rack_id.clone()) {
            return Err(format!("{at}.id: {rack_id} is used twice"));
        }
        if rack.units == 0 || rack.units > MAX_UNITS {
            return Err(format!("{at}.units: {} given; 1–{MAX_UNITS} allowed", rack.units));
        }
        let info = rack.node_info.as_ref();
        if info.is_some_and(|i| i.len() > MAX_NODES) {
            return Err(format!("{at}.node_info: more than {MAX_NODES} nodes"));
        }
        let mut devices = Vec::new();
        for (di, d) in rack.devices.iter().enumerate() {
            let device = device(&format!("{at}.devices[{di}]"), d, rack.units)?;
            if !seen_ids.insert(device.id.clone()) {
                return Err(format!("{at}.devices[{di}].id: {} is used twice", device.id));
            }
            if let Some(name) = &device.node {
                let own = Node { name: name.clone(), ip: device.ip.clone(), model: Some(device.label.clone()), cpu: device.cpu, gpu: device.gpu.clone(), arch: device.arch.clone(), ..Node::default() };
                merge(&mut nodes, own);
            }
            for name in &device.node_candidates {
                merge(&mut nodes, Node { name: name.clone(), ..Node::default() });
            }
            devices.push(device);
        }
        for (name, n) in info.into_iter().flatten() {
            let at = format!("{at}.node_info.{name}");
            let from_info = Node {
                name: node_name(&at, name)?,
                ip: ipv4(&format!("{at}.ip"), &n.ip)?,
                role: optional(&format!("{at}.role"), &n.role, 100)?,
                model: optional(&format!("{at}.model"), &n.model, 200)?,
                cpu: n.cpu,
                mem_gi: n.mem_gi,
                gpu: optional(&format!("{at}.gpu"), &n.gpu, 200)?.filter(|g| g != "-"),
                arch: None,
            };
            merge(&mut nodes, from_info);
        }
        overlaps.extend(overlapping(&rack_id, &devices));
        racks.push(Rack { id: rack_id, name: text(&format!("{at}.name"), &rack.name, 100)?, role: optional(&format!("{at}.role"), &rack.role, 200)?, units: rack.units, devices });
    }
    if nodes.len() > MAX_NODES {
        return Err(format!("more than {MAX_NODES} nodes named"));
    }
    Ok(Layout { version: text("version", &raw.version, 64)?, title: text("title", &raw.title, 300)?, sources, notes: optional("notes", &raw.notes, 4000)?, power, racks, nodes, overlaps })
}

fn power(raw: Option<&BTreeMap<String, serde_json::Value>>) -> Result<Vec<Pair>, String> {
    let Some(raw) = raw else { return Ok(Vec::new()) };
    if raw.len() > MAX_POWER {
        return Err(format!("power: more than {MAX_POWER} entries"));
    }
    // Text entries only; anything else in there is not for the page.
    raw.iter()
        .filter_map(|(k, v)| v.as_str().map(|v| (k, v)))
        .map(|(k, v)| Ok(Pair { label: text("power", k, 100)?, value: text(&format!("power.{k}"), v, 500)? }))
        .collect()
}

fn device(at: &str, d: &RawDevice, units: u32) -> Result<Device, String> {
    let h = d.h.unwrap_or(1);
    if h == 0 || h > units {
        return Err(format!("{at}.h: {h} given; 1–{units} allowed in this rack"));
    }
    if let Some(u) = d.u {
        // `u` is the top slot: the device reaches down to u - h + 1.
        if u == 0 || u > units || u < h {
            return Err(format!("{at}.u: a device {h}U high whose top slot is U{u} does not fit U1–U{units}"));
        }
    }
    let candidates = d.node_candidates.as_deref().unwrap_or_default();
    if candidates.len() > MAX_CANDIDATES {
        return Err(format!("{at}.node_candidates: more than {MAX_CANDIDATES}"));
    }
    let mut node_candidates = Vec::new();
    for (i, name) in candidates.iter().enumerate() {
        let name = node_name(&format!("{at}.node_candidates[{i}]"), name.trim())?;
        if !node_candidates.contains(&name) {
            node_candidates.push(name);
        }
    }
    let node = match d.node.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        Some(n) => Some(node_name(&format!("{at}.node"), n)?),
        None => None,
    };
    let kind = d.kind.as_deref().map(|k| k.trim().to_ascii_lowercase()).unwrap_or_default();
    Ok(Device {
        id: id(&format!("{at}.id"), &d.id)?,
        label: text(&format!("{at}.label"), &d.label, 200)?,
        kind: if TYPES.contains(&kind.as_str()) { kind } else { "other".into() },
        u: d.u,
        h,
        u_confirmed: d.u_confirmed,
        // A confirmed node leaves nothing to choose between.
        node_candidates: if node.is_some() { Vec::new() } else { node_candidates },
        node,
        ip: ipv4(&format!("{at}.ip"), &d.ip)?,
        bmc: ipv4(&format!("{at}.bmc"), &d.bmc)?,
        gpu: optional(&format!("{at}.gpu"), &d.gpu, 200)?,
        cpu: d.cpu,
        arch: optional(&format!("{at}.arch"), &d.arch, 32)?,
        desc: optional(&format!("{at}.desc"), &d.desc, 1000)?,
        order_hint: d.order_hint,
    })
}

/// Add what `more` knows to the node of the same name (what is already
/// known stays), or add the node.
fn merge(nodes: &mut Vec<Node>, more: Node) {
    let Some(have) = nodes.iter_mut().find(|n| n.name == more.name) else {
        nodes.push(more);
        return;
    };
    have.ip = have.ip.take().or(more.ip);
    have.role = have.role.take().or(more.role);
    have.model = have.model.take().or(more.model);
    have.cpu = have.cpu.or(more.cpu);
    have.mem_gi = have.mem_gi.or(more.mem_gi);
    have.gpu = have.gpu.take().or(more.gpu);
    have.arch = have.arch.take().or(more.arch);
}

fn overlapping(rack: &str, devices: &[Device]) -> Vec<Overlap> {
    let span = |d: &Device| d.u.map(|u| (u + 1 - d.h, u));
    let mut out = Vec::new();
    for (i, a) in devices.iter().enumerate() {
        for b in &devices[i + 1..] {
            if let (Some((a_low, a_top)), Some((b_low, b_top))) = (span(a), span(b)) {
                if a_low <= b_top && b_low <= a_top {
                    out.push(Overlap { rack: rack.to_string(), first: a.id.clone(), second: b.id.clone() });
                }
            }
        }
    }
    out
}
