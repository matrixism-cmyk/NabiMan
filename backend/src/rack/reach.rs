//! Whether a device that is not a Kubernetes node answers.
//!
//! A device with an address (`ip`) and no node is looked at in one of two
//! ways, in this order:
//!
//! 1. Prometheus already probes it (blackbox `probe_success`): that answer
//!    is used and nothing is sent from here. An ICMP probe's `instance` is
//!    the bare address; a TCP probe's is `address:port`.
//! 2. Otherwise NabiMan asks once itself (see `probe`): one ICMP echo, or
//!    one TCP connect when the layout says `check: "tcp:<port>"`.
//!
//! What NabiMan asks itself is kept for `TTL` per address and shared by
//! every caller, so open pages never multiply what leaves this server: one
//! echo or one connect per device per `TTL`, at most `CONCURRENCY` at a
//! time, at most `MAX_TARGETS` addresses, a whole round within
//! `ROUND_DEADLINE`.

use std::collections::{HashMap, HashSet};
use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::future::BoxFuture;
use futures::StreamExt;

use super::check::{target_address, Check};
use super::layout::Layout;
use super::prometheus::{Metric, Samples};
use super::status::Level;

pub(crate) const TTL: Duration = Duration::from_secs(30);
pub(crate) const MAX_TARGETS: usize = 128;
pub(crate) const CONCURRENCY: usize = 16;
pub(crate) const ROUND_DEADLINE: Duration = Duration::from_secs(6);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Method {
    Ping,
    Tcp(u16),
}

impl Method {
    fn name(&self) -> String {
        match self {
            Method::Ping => "ping".into(),
            Method::Tcp(port) => format!("tcp:{port}"),
        }
    }
}

pub(crate) type Target = (Ipv4Addr, Method);

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Outcome {
    Reply { ms: Option<f64> },
    NoReply,
    /// It could not be asked; why (`ping_missing`, `ping_denied`, `no_route`, `ping_failed`, `not_in_time`).
    Unavailable(&'static str),
}

/// Whatever asks a device. The server's own is `probe::SystemProber`.
pub(crate) trait Prober: Send + Sync {
    fn probe(&self, target: Target) -> BoxFuture<'static, Outcome>;
}

/// What is known about one device that is no node.
#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) struct Finding {
    pub level: Level,
    /// Why there is no answer to show: `no_address`, `check_none`,
    /// `address_refused`, `over_limit`, or an `Outcome::Unavailable` reason.
    pub reason: Option<&'static str>,
    pub latency_ms: Option<f64>,
    /// `prometheus-icmp`, `prometheus-tcp`, `ping`, `tcp:<port>`.
    pub checked_by: Option<String>,
    pub checked_at: Option<String>,
}

impl Finding {
    fn because(level: Level, reason: &'static str) -> Self {
        Finding { level, reason: Some(reason), ..Finding::default() }
    }
}

/// Every non-node device's finding, by device id.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Reach {
    pub found: HashMap<String, Finding>,
    /// Devices past `MAX_TARGETS`, not asked.
    pub over_limit: usize,
}

/// What Prometheus settled, and what is left for NabiMan to ask.
#[derive(Debug, Default)]
pub(crate) struct Plan {
    pub reach: Reach,
    /// Device id and what to ask; several devices may share a target.
    pub wanted: Vec<(String, Target)>,
}

fn ms(seconds: f64) -> f64 {
    (seconds * 100_000.0).round() / 100.0
}

/// Prometheus' own probes, by `instance`: answered or not, and how long it took.
struct Probes<'a> {
    icmp: HashMap<&'a str, (bool, Option<f64>)>,
    /// By address: each probed `address:port` of it.
    tcp: HashMap<&'a str, Vec<(u16, bool, Option<f64>)>>,
}

impl<'a> Probes<'a> {
    fn from(samples: &'a Samples) -> Self {
        let took: HashMap<&str, f64> = samples.of(Metric::ProbeDuration).iter().filter_map(|s| Some((s.labels.get("instance")?.as_str(), s.value))).collect();
        let mut probes = Probes { icmp: HashMap::new(), tcp: HashMap::new() };
        for s in samples.of(Metric::ProbeSuccess) {
            let Some(instance) = s.labels.get("instance").map(String::as_str) else { continue };
            let seen = (s.value >= 1.0, took.get(instance).map(|t| ms(*t)));
            match instance.rsplit_once(':') {
                None => {
                    probes.icmp.insert(instance, seen);
                }
                Some((address, port)) => {
                    if let Ok(port) = port.parse::<u16>() {
                        probes.tcp.entry(address).or_default().push((port, seen.0, seen.1));
                    }
                }
            }
        }
        probes
    }

    /// The probe that speaks for `ip` under `check`, if Prometheus has one:
    /// (answered, latency, which kind).
    fn of(&self, ip: &str, check: Check) -> Option<(bool, Option<f64>, &'static str)> {
        let ports = self.tcp.get(ip).map(Vec::as_slice).unwrap_or_default();
        match check {
            // A declared port: only a probe of that very port will do.
            Check::Tcp(port) => ports.iter().find(|p| p.0 == port).map(|p| (p.1, p.2, "prometheus-tcp")),
            Check::Ping => self.icmp.get(ip).map(|p| (p.0, p.1, "prometheus-icmp")).or_else(|| {
                // Only TCP probes exist: any port that opens shows the machine answers.
                let best = ports.iter().filter(|p| p.1).min_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal)).or(ports.first())?;
                Some((best.1, best.2, "prometheus-tcp"))
            }),
            Check::None => None,
        }
    }
}

/// Sort every non-node device: settled without asking (no address, not to
/// be checked, Prometheus knows), or to be asked by NabiMan.
pub(crate) fn plan(layout: &Layout, samples: Option<&Samples>, stamp: &str) -> Plan {
    let probes = samples.map(Probes::from);
    let mut plan = Plan::default();
    let mut targets: HashSet<Target> = HashSet::new();
    for device in layout.racks.iter().flat_map(|r| &r.devices).filter(|d| d.nodes().is_empty()) {
        let finding = match (&device.ip, device.check) {
            (_, Check::None) => Finding::because(Level::None, "check_none"),
            (None, _) => Finding::because(Level::None, "no_address"),
            (Some(ip), check) => match target_address(ip) {
                None => Finding::because(Level::Unknown, "address_refused"),
                Some(address) => {
                    if let Some((answered, latency_ms, by)) = probes.as_ref().and_then(|p| p.of(ip, check)) {
                        Finding { level: if answered { Level::Ok } else { Level::NoReply }, reason: None, latency_ms: latency_ms.filter(|_| answered), checked_by: Some(by.into()), checked_at: Some(stamp.to_string()) }
                    } else {
                        let target = (address, if let Check::Tcp(port) = check { Method::Tcp(port) } else { Method::Ping });
                        if targets.contains(&target) || targets.len() < MAX_TARGETS {
                            targets.insert(target);
                            plan.wanted.push((device.id.clone(), target));
                            continue;
                        }
                        plan.reach.over_limit += 1;
                        Finding::because(Level::Unknown, "over_limit")
                    }
                }
            },
        };
        plan.reach.found.insert(device.id.clone(), finding);
    }
    plan
}

impl Plan {
    /// Each distinct thing to ask.
    pub(crate) fn targets(&self) -> Vec<Target> {
        let mut seen = HashSet::new();
        self.wanted.iter().map(|(_, t)| *t).filter(|t| seen.insert(*t)).collect()
    }

    /// The plan with what was asked filled in.
    pub(crate) fn finish(mut self, asked: &HashMap<Target, Asked>) -> Reach {
        for (id, target) in self.wanted {
            let by = Some(target.1.name());
            let finding = match asked.get(&target) {
                Some(Asked { outcome: Outcome::Reply { ms }, stamp }) => Finding { level: Level::Ok, reason: None, latency_ms: ms.map(|v| (v * 100.0).round() / 100.0), checked_by: by, checked_at: Some(stamp.clone()) },
                Some(Asked { outcome: Outcome::NoReply, stamp }) => Finding { level: Level::NoReply, reason: None, latency_ms: None, checked_by: by, checked_at: Some(stamp.clone()) },
                Some(Asked { outcome: Outcome::Unavailable(why), stamp }) => Finding { level: Level::Unknown, reason: Some(why), latency_ms: None, checked_by: by, checked_at: Some(stamp.clone()) },
                None => Finding { level: Level::Unknown, reason: Some("not_in_time"), latency_ms: None, checked_by: by, checked_at: None },
            };
            self.reach.found.insert(id, finding);
        }
        self.reach
    }
}

/// One target's answer and when it was asked (RFC 3339).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Asked {
    pub outcome: Outcome,
    pub stamp: String,
}

struct Kept {
    at: Instant,
    asked: Asked,
}

struct Inner {
    prober: Arc<dyn Prober>,
    ttl: Duration,
    /// Held for a whole round, so callers arriving together share it.
    kept: tokio::sync::Mutex<HashMap<Target, Kept>>,
}

/// NabiMan's own answers, each at most `ttl` old.
pub(crate) struct Rounds(Arc<Inner>);

impl Rounds {
    pub(crate) fn new(prober: Arc<dyn Prober>, ttl: Duration) -> Self {
        Rounds(Arc::new(Inner { prober, ttl, kept: tokio::sync::Mutex::new(HashMap::new()) }))
    }

    /// The answer for each of `wanted` (at most `MAX_TARGETS` are looked
    /// at), asking only those not asked within `ttl`. The round runs as its
    /// own task: a caller that goes away does not cut it short, so the next
    /// caller finds its answers instead of asking again.
    pub(crate) async fn answers(&self, mut wanted: Vec<Target>) -> HashMap<Target, Asked> {
        if wanted.is_empty() {
            return HashMap::new();
        }
        wanted.truncate(MAX_TARGETS);
        let inner = self.0.clone();
        tokio::spawn(async move { inner.round(wanted).await }).await.unwrap_or_default()
    }
}

impl Inner {
    async fn round(&self, wanted: Vec<Target>) -> HashMap<Target, Asked> {
        let mut kept = self.kept.lock().await;
        kept.retain(|target, k| k.at.elapsed() < self.ttl || wanted.contains(target));
        let due: Vec<Target> = wanted.iter().copied().filter(|t| kept.get(t).is_none_or(|k| k.at.elapsed() >= self.ttl)).collect();
        if !due.is_empty() {
            let stamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
            let at = Instant::now();
            let deadline = tokio::time::Instant::now() + ROUND_DEADLINE;
            let mut asking = futures::stream::iter(due.iter().copied().map(|target| {
                let answer = self.prober.probe(target);
                async move { (target, answer.await) }
            }))
            .buffer_unordered(CONCURRENCY);
            let mut answered: HashMap<Target, Outcome> = HashMap::new();
            while let Ok(Some((target, outcome))) = tokio::time::timeout_at(deadline, asking.next()).await {
                answered.insert(target, outcome);
            }
            drop(asking);
            for target in due {
                // One that did not finish in time is not asked again before `ttl` either.
                let outcome = answered.remove(&target).unwrap_or(Outcome::Unavailable("not_in_time"));
                kept.insert(target, Kept { at, asked: Asked { outcome, stamp: stamp.clone() } });
            }
        }
        wanted.iter().filter_map(|t| kept.get(t).map(|k| (*t, k.asked.clone()))).collect()
    }
}
