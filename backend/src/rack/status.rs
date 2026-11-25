//! The state of each node and device, put together from what Prometheus
//! answered. Names and addresses from the layout are looked up among the
//! labels of the answers; nothing here builds a query.

use std::collections::{BTreeMap, HashMap};

use serde::Serialize;

use super::layout::Layout;
use super::reach::{Finding, Reach, MAX_TARGETS};
use super::prometheus::{Metric, Sample, Samples};

/// At or above these a node is `warn`.
pub(crate) const WARN_TEMP_C: f64 = 80.0;
pub(crate) const WARN_MEMORY_PCT: f64 = 90.0;
pub(crate) const WARN_DISK_PCT: f64 = 90.0;

/// Worst last, so the worst of several is their maximum.
#[derive(Serialize, Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Level {
    /// Nothing is collected for this device.
    #[default]
    None,
    Ok,
    /// There is no answer about it (Prometheus has none, or it could not be asked).
    Unknown,
    /// A device that is no node was asked and did not answer (see `reach`).
    /// Not `Down`: that is a Kubernetes node that is NotReady.
    NoReply,
    Warn,
    Down,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub(crate) struct Gpu {
    pub index: u32,
    /// A MIG slice of the GPU: its instance id and profile.
    pub mig_id: Option<u32>,
    pub mig_profile: Option<String>,
    pub model: Option<String>,
    pub util_pct: Option<f64>,
    pub temp_c: Option<f64>,
    pub power_w: Option<f64>,
    pub mem_used_mib: Option<f64>,
    pub mem_total_mib: Option<f64>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub(crate) struct NodeStatus {
    pub level: Level,
    /// Why it is `warn`: `board_temp`, `gpu_temp`, `memory`, `disk`.
    pub reasons: Vec<&'static str>,
    pub ready: Option<bool>,
    pub cpu_pct: Option<f64>,
    pub mem_pct: Option<f64>,
    pub disk_pct: Option<f64>,
    pub temp_c: Option<f64>,
    pub gpus: Vec<Gpu>,
}

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
pub(crate) struct DeviceStatus {
    /// The worst of its nodes. Without a node: `ok` (it answers), `noreply`,
    /// `unknown` (it could not be asked) or `none` (nothing to ask; see `reason`).
    pub level: Level,
    /// The nodes it shows: its own, or the ones it may be.
    pub nodes: Vec<String>,
    /// Not one of ours: no answer from it is not our fault to fix.
    pub external: bool,
    /// Without a node, why there is no answer to show: `no_address`,
    /// `check_none`, `address_refused`, `over_limit`, `not_in_time`,
    /// `ping_missing`, `ping_denied`, `no_route`, `ping_failed`.
    pub reason: Option<&'static str>,
    pub latency_ms: Option<f64>,
    /// `prometheus-icmp`, `prometheus-tcp`, `ping`, `tcp:<port>`.
    pub checked_by: Option<String>,
    pub checked_at: Option<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
pub(crate) struct Summary {
    pub nodes_total: usize,
    pub nodes_ready: usize,
    /// Physical GPUs (a GPU cut into MIG slices is one).
    pub gpus: usize,
    pub warnings: usize,
    pub down: usize,
    pub max_temp_c: Option<f64>,
    /// Devices that are no node and were asked whether they answer.
    pub probed: usize,
    /// Of those: answered.
    pub reachable: usize,
    /// Of those: ours, and did not answer.
    pub noreply: usize,
    /// Of those: somebody else's, and did not answer.
    pub external_noreply: usize,
    /// Devices not asked because more than the limit have an address.
    pub over_limit: usize,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub(crate) struct Thresholds {
    pub temp_c: f64,
    pub memory_pct: f64,
    pub disk_pct: f64,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub(crate) struct Status {
    pub collected_at: String,
    /// `ok`, `partial` (some queries failed), `unreachable`, `not_configured`.
    pub source: &'static str,
    pub message: Option<String>,
    pub layout_version: String,
    pub nodes: BTreeMap<String, NodeStatus>,
    pub devices: BTreeMap<String, DeviceStatus>,
    pub summary: Summary,
    pub thresholds: Thresholds,
}

/// What was collected, or why nothing was.
pub(crate) enum Source<'a> {
    Collected(&'a Samples),
    Unreachable(&'a str),
    NotConfigured,
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

/// The largest value per address in a node_exporter answer (`instance` is `<ip>:<port>`).
fn by_address(samples: &[Sample]) -> HashMap<&str, f64> {
    let mut out: HashMap<&str, f64> = HashMap::new();
    for s in samples {
        let Some(instance) = s.labels.get("instance") else { continue };
        let address = instance.rsplit_once(':').map_or(instance.as_str(), |(host, _)| host);
        out.entry(address).and_modify(|v| *v = v.max(s.value)).or_insert(s.value);
    }
    out
}

type GpuKey = (u32, Option<u32>);

/// Each node's GPUs, by DCGM `Hostname`, from the five GPU answers.
fn gpus_by_host(samples: &Samples) -> HashMap<&str, BTreeMap<GpuKey, Gpu>> {
    let mut hosts: HashMap<&str, BTreeMap<GpuKey, Gpu>> = HashMap::new();
    let mut free: HashMap<(&str, GpuKey), f64> = HashMap::new();
    for metric in [Metric::GpuUtil, Metric::GpuTemp, Metric::GpuPower, Metric::GpuMemUsed, Metric::GpuMemFree] {
        for s in samples.of(metric) {
            let (Some(host), Some(index)) = (s.labels.get("Hostname"), s.labels.get("gpu").and_then(|g| g.parse::<u32>().ok())) else { continue };
            let label = |name: &str| s.labels.get(name).filter(|v| !v.is_empty());
            let key = (index, label("GPU_I_ID").and_then(|v| v.parse().ok()));
            let gpu = hosts.entry(host.as_str()).or_default().entry(key).or_insert_with(|| Gpu {
                index,
                mig_id: key.1,
                mig_profile: None,
                model: None,
                util_pct: None,
                temp_c: None,
                power_w: None,
                mem_used_mib: None,
                mem_total_mib: None,
            });
            gpu.model = gpu.model.take().or_else(|| label("modelName").map(|m| m.chars().take(100).collect()));
            gpu.mig_profile = gpu.mig_profile.take().or_else(|| label("GPU_I_PROFILE").map(|m| m.chars().take(40).collect()));
            match metric {
                Metric::GpuUtil => gpu.util_pct = Some(round1(s.value)),
                Metric::GpuTemp => gpu.temp_c = Some(round1(s.value)),
                Metric::GpuPower => gpu.power_w = Some(round1(s.value)),
                Metric::GpuMemUsed => gpu.mem_used_mib = Some(s.value.round()),
                _ => {
                    free.insert((host.as_str(), key), s.value);
                }
            }
        }
    }
    for (host, gpus) in hosts.iter_mut() {
        for (key, gpu) in gpus.iter_mut() {
            gpu.mem_total_mib = gpu.mem_used_mib.zip(free.get(&(*host, *key)).copied()).map(|(used, free)| (used + free).round());
        }
    }
    hosts
}

fn unknown_node() -> NodeStatus {
    NodeStatus { level: Level::Unknown, reasons: Vec::new(), ready: None, cpu_pct: None, mem_pct: None, disk_pct: None, temp_c: None, gpus: Vec::new() }
}

fn judge(node: &mut NodeStatus) {
    if node.temp_c.is_some_and(|t| t >= WARN_TEMP_C) {
        node.reasons.push("board_temp");
    }
    if node.gpus.iter().any(|g| g.temp_c.is_some_and(|t| t >= WARN_TEMP_C)) {
        node.reasons.push("gpu_temp");
    }
    if node.mem_pct.is_some_and(|m| m >= WARN_MEMORY_PCT) {
        node.reasons.push("memory");
    }
    if node.disk_pct.is_some_and(|d| d >= WARN_DISK_PCT) {
        node.reasons.push("disk");
    }
    node.level = match node.ready {
        Some(false) => Level::Down,
        _ if !node.reasons.is_empty() => Level::Warn,
        Some(true) => Level::Ok,
        None => Level::Unknown,
    };
}

fn nodes_from(layout: &Layout, samples: &Samples) -> BTreeMap<String, NodeStatus> {
    let ready: HashMap<&str, bool> = samples.of(Metric::Ready).iter().filter_map(|s| Some((s.labels.get("node")?.as_str(), s.value >= 1.0))).collect();
    let (cpu, memory, disk, temp) = (by_address(samples.of(Metric::Cpu)), by_address(samples.of(Metric::Memory)), by_address(samples.of(Metric::Disk)), by_address(samples.of(Metric::BoardTemp)));
    let mut gpus = gpus_by_host(samples);
    layout
        .nodes
        .iter()
        .map(|n| {
            let at = |values: &HashMap<&str, f64>| n.ip.as_deref().and_then(|ip| values.get(ip)).map(|v| round1(*v));
            let mut node = NodeStatus {
                ready: ready.get(n.name.as_str()).copied(),
                cpu_pct: at(&cpu).map(|v| v.clamp(0.0, 100.0)),
                mem_pct: at(&memory).map(|v| v.clamp(0.0, 100.0)),
                disk_pct: at(&disk).map(|v| v.clamp(0.0, 100.0)),
                temp_c: at(&temp),
                gpus: gpus.remove(n.name.as_str()).map(|g| g.into_values().collect()).unwrap_or_default(),
                ..unknown_node()
            };
            judge(&mut node);
            (n.name.clone(), node)
        })
        .collect()
}

/// The page's status for `layout`: its nodes from `source`, its other
/// devices from `reach`, stamped `collected_at`.
pub(crate) fn assemble(layout: &Layout, source: Source, reach: &Reach, collected_at: String) -> Status {
    let (nodes, name, message) = match source {
        Source::Collected(samples) => {
            let partial = !samples.failed.is_empty();
            (nodes_from(layout, samples), if partial { "partial" } else { "ok" }, partial.then(|| format!("no answer for: {}", samples.failed.join(", "))))
        }
        Source::Unreachable(why) => (layout.nodes.iter().map(|n| (n.name.clone(), unknown_node())).collect(), "unreachable", Some(why.to_string())),
        Source::NotConfigured => (layout.nodes.iter().map(|n| (n.name.clone(), unknown_node())).collect(), "not_configured", None),
    };
    let devices: BTreeMap<String, DeviceStatus> = layout
        .racks
        .iter()
        .flat_map(|r| &r.devices)
        .map(|d| {
            let names = d.nodes();
            let status = match names.iter().map(|n| nodes.get(*n).map_or(Level::Unknown, |s| s.level)).max() {
                Some(level) => DeviceStatus { level, nodes: names.into_iter().map(String::from).collect(), external: d.external, ..DeviceStatus::default() },
                None => {
                    let Finding { level, reason, latency_ms, checked_by, checked_at } = reach.found.get(&d.id).cloned().unwrap_or_default();
                    DeviceStatus { level, nodes: Vec::new(), external: d.external, reason, latency_ms, checked_by, checked_at }
                }
            };
            (d.id.clone(), status)
        })
        .collect();
    let probed = || devices.values().filter(|d| d.checked_by.is_some());
    let message = match reach.over_limit {
        0 => message,
        n => {
            let said = format!("{n} devices were not checked for an answer: more than {MAX_TARGETS} have an address");
            Some(message.map_or(said.clone(), |m| format!("{m}; {said}")))
        }
    };
    let temps = nodes.values().flat_map(|n| n.temp_c.into_iter().chain(n.gpus.iter().filter_map(|g| g.temp_c)));
    let summary = Summary {
        nodes_total: nodes.len(),
        nodes_ready: nodes.values().filter(|n| n.ready == Some(true)).count(),
        gpus: nodes.values().map(|n| n.gpus.iter().map(|g| g.index).collect::<std::collections::BTreeSet<_>>().len()).sum(),
        warnings: nodes.values().filter(|n| n.level == Level::Warn).count(),
        down: nodes.values().filter(|n| n.level == Level::Down).count(),
        max_temp_c: temps.fold(None, |max: Option<f64>, t| Some(max.map_or(t, |m| m.max(t)))),
        probed: probed().count(),
        reachable: probed().filter(|d| d.level == Level::Ok).count(),
        noreply: probed().filter(|d| d.level == Level::NoReply && !d.external).count(),
        external_noreply: probed().filter(|d| d.level == Level::NoReply && d.external).count(),
        over_limit: reach.over_limit,
    };
    Status {
        collected_at,
        source: name,
        message,
        layout_version: layout.version.clone(),
        nodes,
        devices,
        summary,
        thresholds: Thresholds { temp_c: WARN_TEMP_C, memory_pct: WARN_MEMORY_PCT, disk_pct: WARN_DISK_PCT },
    }
}
