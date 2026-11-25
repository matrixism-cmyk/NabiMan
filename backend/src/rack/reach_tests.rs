use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::future::BoxFuture;

use super::check::{target_address, Check};
use super::layout::{parse, Layout};
use super::prometheus::Metric;
use super::reach::*;
use super::status::{assemble, Level, Source};
use super::status_tests::{samples_with, vector};

/// A prober that sends nothing: it answers from a table (no entry: no
/// reply), after `delay`, and remembers what it was asked.
pub(super) struct FakeProber {
    pub asked: Mutex<Vec<Target>>,
    pub answers: HashMap<Target, Outcome>,
    pub delay: Duration,
    pub running: AtomicUsize,
    pub most_at_once: AtomicUsize,
}

impl FakeProber {
    pub(super) fn new(answers: &[(Target, Outcome)], delay: Duration) -> Arc<Self> {
        Arc::new(FakeProber { asked: Mutex::new(Vec::new()), answers: answers.iter().cloned().collect(), delay, running: AtomicUsize::new(0), most_at_once: AtomicUsize::new(0) })
    }
    pub(super) fn count(&self) -> usize {
        self.asked.lock().unwrap().len()
    }
}

/// Counts through an `Arc` so the future owns what it touches.
struct Shared(Arc<FakeProber>);

impl Prober for Shared {
    fn probe(&self, target: Target) -> BoxFuture<'static, Outcome> {
        let me = self.0.clone();
        me.asked.lock().unwrap().push(target);
        Box::pin(async move {
            let now = me.running.fetch_add(1, Ordering::SeqCst) + 1;
            me.most_at_once.fetch_max(now, Ordering::SeqCst);
            tokio::time::sleep(me.delay).await;
            me.running.fetch_sub(1, Ordering::SeqCst);
            me.answers.get(&target).cloned().unwrap_or(Outcome::NoReply)
        })
    }
}

pub(super) fn rounds(prober: &Arc<FakeProber>, ttl: Duration) -> Rounds {
    Rounds::new(Arc::new(Shared(prober.clone())), ttl)
}

/// For tests about something else: nothing answers, nothing leaves.
pub(super) fn quiet() -> Rounds {
    rounds(&FakeProber::new(&[], Duration::ZERO), Duration::from_secs(60))
}

fn ip(text: &str) -> Ipv4Addr {
    text.parse().unwrap()
}

/// One rack of ours and one of somebody else's, with every kind of device
/// the reachability rules tell apart.
const ROOM: &str = r#"{"version": "r1", "racks": [
 {"id": "R1", "units": 42, "devices": [
  {"id": "node", "label": "Server", "type": "server", "u": 40, "node": "mec-cp01", "ip": "10.0.0.201"},
  {"id": "probed", "label": "Core", "type": "5g", "u": 38, "ip": "10.0.0.5"},
  {"id": "probed-down", "label": "Old switch", "type": "switch", "u": 37, "ip": "10.0.0.6"},
  {"id": "tcp-probed", "label": "Mux", "type": "5g", "u": 36, "ip": "10.0.0.7"},
  {"id": "sw", "label": "Switch", "type": "switch", "u": 35, "ip": "10.0.0.1"},
  {"id": "silent", "label": "Mini PC", "u": 34, "ip": "10.0.0.2"},
  {"id": "fw", "label": "Firewall", "type": "firewall", "u": 33, "ip": "203.0.113.9", "check": "tcp:3344"},
  {"id": "fw-probed", "label": "Firewall 2", "type": "firewall", "u": 32, "ip": "10.0.0.7", "check": "tcp:22"},
  {"id": "quiet", "label": "PDU", "u": 31, "ip": "10.0.0.3", "check": "none"},
  {"id": "bare", "label": "Storage", "type": "storage", "u": 30},
  {"id": "nowhere", "label": "Typo", "u": 29, "ip": "0.0.0.0"},
  {"id": "guest", "label": "Guest", "u": 28, "ip": "10.0.0.4", "external": true}
 ]},
 {"id": "R2", "units": 42, "external": true, "devices": [
  {"id": "theirs", "label": "Theirs", "u": 40, "ip": "10.9.0.1"},
  {"id": "theirs-up", "label": "Theirs too", "u": 39, "ip": "10.9.0.2"},
  {"id": "lent", "label": "Ours in their rack", "u": 38, "ip": "10.9.0.3", "external": false}
 ]}
]}"#;

fn room() -> Layout {
    parse(ROOM.as_bytes()).unwrap()
}

fn probes() -> super::prometheus::Samples {
    samples_with(&|m| match m {
        Metric::ProbeSuccess => Some(vector(&[
            (&[("instance", "10.0.0.5")], "1"),
            (&[("instance", "10.0.0.6")], "0"),
            (&[("instance", "10.0.0.7:22")], "1"),
            (&[("instance", "10.0.0.7:5201")], "0"),
            (&[("instance", "10.0.0.201")], "1"),
            (&[("instance", "http://10.0.0.2/")], "1"),
        ])),
        Metric::ProbeDuration => Some(vector(&[(&[("instance", "10.0.0.5")], "0.000420691"), (&[("instance", "10.0.0.6")], "1.0"), (&[("instance", "10.0.0.7:22")], "0.0013")])),
        _ => None,
    })
}

fn answers() -> Vec<(Target, Outcome)> {
    vec![
        ((ip("10.0.0.1"), Method::Ping), Outcome::Reply { ms: Some(0.612) }),
        ((ip("203.0.113.9"), Method::Tcp(3344)), Outcome::Reply { ms: Some(11.456) }),
        ((ip("10.9.0.2"), Method::Ping), Outcome::Reply { ms: None }),
    ]
}

fn asked(answers: &[(Target, Outcome)], targets: &[Target]) -> HashMap<Target, Asked> {
    let table: HashMap<Target, Outcome> = answers.iter().cloned().collect();
    targets.iter().map(|t| (*t, Asked { outcome: table.get(t).cloned().unwrap_or(Outcome::NoReply), stamp: "2026-11-25T12:00:05Z".into() })).collect()
}

#[test]
fn check_is_ping_tcp_or_none_and_anything_else_names_the_device() {
    let device = |check: &str| format!(r#"{{"racks":[{{"id":"R1","units":42,"devices":[{{"id":"fw-1","label":"F","u":1,"ip":"10.0.0.1","check":{check}}}]}}]}}"#);
    let read = |check: &str| parse(device(check).as_bytes()).map(|l| l.racks[0].devices[0].check);
    assert_eq!(read(r#""ping""#), Ok(Check::Ping));
    assert_eq!(read("null"), Ok(Check::Ping), "no field: ping");
    assert_eq!(read(r#""  ""#), Ok(Check::Ping));
    assert_eq!(read(r#"" TCP:443 ""#), Ok(Check::Tcp(443)));
    assert_eq!((read(r#""tcp:1""#), read(r#""tcp:65535""#)), (Ok(Check::Tcp(1)), Ok(Check::Tcp(65535))));
    assert_eq!(read(r#""none""#), Ok(Check::None));
    for bad in ["icmp", "tcp", "tcp:", "tcp:0", "tcp:65536", "tcp:22x", "tcp:+22", "tcp:-1", "tcp: 22", "tcp:22:23", "udp:53", "ping -f", "tcp:000022", "snmp"] {
        let fault = read(&format!("\"{bad}\"")).expect_err(bad);
        assert!(fault.contains("racks[0].devices[0].check (fw-1)"), "{bad}: {fault}");
        assert!(fault.contains("tcp:<port>"), "{bad}: {fault}");
    }
    assert!(parse(device("22").as_bytes()).is_err(), "a number is not a check");
    let json = serde_json::to_value(parse(device(r#""tcp:3344""#).as_bytes()).unwrap()).unwrap();
    assert_eq!(json["racks"][0]["devices"][0]["check"], "tcp:3344");
    assert_eq!((Check::Ping.name(), Check::None.name()), ("ping".to_string(), "none".to_string()));
}

#[test]
fn external_is_the_devices_word_then_the_racks_then_its_type() {
    let layout = room();
    let external = |id: &str| layout.racks.iter().flat_map(|r| &r.devices).find(|d| d.id == id).unwrap().external;
    assert_eq!((layout.racks[0].external, layout.racks[1].external), (false, true));
    assert!(!external("sw") && external("guest"), "a device may say so itself");
    assert!(external("theirs") && !external("lent"), "the rack's word, unless the device has its own");
    let typed = parse(br#"{"racks":[{"id":"R1","units":4,"devices":[{"id":"a","label":"A","type":"external","u":1},{"id":"b","label":"B","type":"external","u":2,"external":false}]}]}"#).unwrap();
    assert_eq!((typed.racks[0].devices[0].external, typed.racks[0].devices[1].external), (true, false), "the type that means somebody else's");
    assert!(parse(br#"{"racks":[{"id":"R1","units":4,"external":"yes","devices":[]}]}"#).is_err(), "a yes/no, not prose");
    let prose = parse(r#"{"racks":[{"id":"R1","units":4,"role":"우리 클러스터 아님","devices":[{"id":"a","label":"A","u":1}]}]}"#.as_bytes()).unwrap();
    assert!(!prose.racks[0].external && !prose.racks[0].devices[0].external, "what the role says in words is not read");
}

#[test]
fn any_machines_address_may_be_asked_but_not_a_non_address() {
    for ok in ["10.0.0.1", "172.20.26.1", "192.168.0.1", "127.0.0.1", "169.254.1.1", "121.147.13.228", "8.8.8.8", "100.64.0.1"] {
        assert_eq!(target_address(ok), Some(ip(ok)), "{ok}");
    }
    for bad in ["0.0.0.0", "0.1.2.3", "255.255.255.255", "224.0.0.1", "239.255.255.250", "010.0.0.1", "10.0.0.1 ", "-c", "::1", "host", "10.0.0.1/24", ""] {
        assert_eq!(target_address(bad), None, "{bad}");
    }
}

#[test]
fn prometheus_probes_settle_a_device_without_anything_being_sent() {
    let samples = probes();
    let plan = plan(&room(), Some(&samples), "2026-11-25T12:00:00Z");
    let found = &plan.reach.found;
    assert!(!found.contains_key("node"), "a node's state is its node's");
    assert_eq!(found["probed"], Finding { level: Level::Ok, reason: None, latency_ms: Some(0.42), checked_by: Some("prometheus-icmp".into()), checked_at: Some("2026-11-25T12:00:00Z".into()) });
    assert_eq!((found["probed-down"].level, found["probed-down"].latency_ms, found["probed-down"].checked_by.as_deref()), (Level::NoReply, None, Some("prometheus-icmp")));
    // Only TCP probes of the address: one that opens is an answer.
    assert_eq!((found["tcp-probed"].level, found["tcp-probed"].latency_ms, found["tcp-probed"].checked_by.as_deref()), (Level::Ok, Some(1.3), Some("prometheus-tcp")));
    assert_eq!((found["fw-probed"].level, found["fw-probed"].checked_by.as_deref()), (Level::Ok, Some("prometheus-tcp")), "the declared port is probed already");
    assert_eq!(found["quiet"], Finding { level: Level::None, reason: Some("check_none"), ..Finding::default() });
    assert_eq!(found["bare"], Finding { level: Level::None, reason: Some("no_address"), ..Finding::default() });
    assert_eq!(found["nowhere"], Finding { level: Level::Unknown, reason: Some("address_refused"), ..Finding::default() });
    let wanted: Vec<(&str, Target)> = plan.wanted.iter().map(|(id, t)| (id.as_str(), *t)).collect();
    assert_eq!(
        wanted,
        [
            ("sw", (ip("10.0.0.1"), Method::Ping)),
            ("silent", (ip("10.0.0.2"), Method::Ping)),
            ("fw", (ip("203.0.113.9"), Method::Tcp(3344))),
            ("guest", (ip("10.0.0.4"), Method::Ping)),
            ("theirs", (ip("10.9.0.1"), Method::Ping)),
            ("theirs-up", (ip("10.9.0.2"), Method::Ping)),
            ("lent", (ip("10.9.0.3"), Method::Ping)),
        ],
        "an http probe of an address is neither kind; a probe of another port does not answer for a declared one"
    );
    // Without Prometheus everything with an address is NabiMan's to ask, each address once.
    let alone = super::reach::plan(&room(), None, "t");
    assert_eq!(alone.wanted.len(), 11);
    assert_eq!(alone.targets().len(), 11, "10.0.0.7 by ping and by tcp:22 are two questions");
}

#[test]
fn what_nabiman_asked_becomes_ok_noreply_or_unknown_with_the_reason() {
    let samples = probes();
    let plan = plan(&room(), Some(&samples), "2026-11-25T12:00:00Z");
    let mut table = answers();
    table.push(((ip("10.0.0.4"), Method::Ping), Outcome::Unavailable("ping_missing")));
    let mut asked = asked(&table, &plan.targets());
    asked.remove(&(ip("10.9.0.3"), Method::Ping));
    let reach = plan.finish(&asked);
    let layout = room();
    let status = assemble(&layout, Source::Collected(&samples), &reach, "2026-11-25T12:00:00Z".into());
    let d = |id: &str| &status.devices[id];
    assert_eq!((d("sw").level, d("sw").latency_ms, d("sw").checked_by.as_deref(), d("sw").checked_at.as_deref(), d("sw").external), (Level::Ok, Some(0.61), Some("ping"), Some("2026-11-25T12:00:05Z"), false));
    assert_eq!((d("fw").level, d("fw").latency_ms, d("fw").checked_by.as_deref()), (Level::Ok, Some(11.46), Some("tcp:3344")));
    assert_eq!((d("silent").level, d("silent").reason, d("silent").latency_ms, d("silent").checked_by.as_deref()), (Level::NoReply, None, None, Some("ping")));
    assert_eq!((d("guest").level, d("guest").reason, d("guest").external), (Level::Unknown, Some("ping_missing"), true), "no tool to ask with is not 'no reply'");
    assert_eq!((d("theirs").level, d("theirs").external), (Level::NoReply, true));
    assert_eq!((d("theirs-up").level, d("theirs-up").latency_ms), (Level::Ok, None));
    assert_eq!((d("lent").level, d("lent").reason, d("lent").checked_at.as_deref()), (Level::Unknown, Some("not_in_time"), None));
    assert_eq!((d("bare").level, d("bare").reason), (Level::None, Some("no_address")));
    assert_eq!((d("quiet").level, d("quiet").reason, d("quiet").checked_by.as_deref()), (Level::None, Some("check_none"), None));
    assert_eq!((d("node").level, d("node").reason, d("node").checked_by.as_deref()), (Level::Ok, None, None), "a node keeps its node's level");
    // probed, probed-down, tcp-probed, sw, silent, fw, fw-probed, guest, theirs, theirs-up, lent
    let s = &status.summary;
    assert_eq!((s.probed, s.reachable, s.noreply, s.external_noreply, s.over_limit), (11, 6, 2, 1, 0));
    assert_eq!((s.warnings, s.down, s.nodes_total), (0, 0, 1), "no reply from a device is not a node down");
    assert_eq!((status.source, status.message.as_deref()), ("ok", None));
    let json = serde_json::to_value(&status).unwrap();
    assert_eq!(json["devices"]["silent"]["level"], "noreply");
    assert_eq!(json["devices"]["bare"], serde_json::json!({"level": "none", "nodes": [], "external": false, "reason": "no_address", "latency_ms": null, "checked_by": null, "checked_at": null}));
}

#[test]
fn past_the_limit_devices_are_unknown_and_the_status_says_so() {
    let devices: Vec<String> = (0..MAX_TARGETS + 3).map(|i| format!(r#"{{"id":"d{i}","label":"D","ip":"10.1.{}.{}"}}"#, i / 200, 1 + i % 200)).collect();
    let text = format!(r#"{{"racks":[{{"id":"R1","units":42,"devices":[{},{{"id":"twin","label":"T","ip":"10.1.0.1"}}]}}]}}"#, devices.join(","));
    let layout = parse(text.as_bytes()).unwrap();
    let plan = plan(&layout, None, "t");
    assert_eq!((plan.targets().len(), plan.wanted.len(), plan.reach.over_limit), (MAX_TARGETS, MAX_TARGETS + 1, 3), "a second device at an address already asked costs nothing");
    assert_eq!(plan.reach.found[&format!("d{}", MAX_TARGETS)], Finding { level: Level::Unknown, reason: Some("over_limit"), ..Finding::default() });
    let reach = plan.finish(&HashMap::new());
    let status = assemble(&layout, Source::NotConfigured, &reach, "t".into());
    assert_eq!(status.summary.over_limit, 3);
    assert_eq!(status.message.as_deref(), Some("3 devices were not checked for an answer: more than 128 have an address"));
    let both = assemble(&layout, Source::Unreachable("cannot connect"), &reach, "t".into());
    assert_eq!(both.message.as_deref(), Some("cannot connect; 3 devices were not checked for an answer: more than 128 have an address"));
}

#[actix_web::test]
async fn callers_arriving_together_share_one_round_and_the_answers_are_kept() {
    let prober = FakeProber::new(&answers(), Duration::from_millis(30));
    let rounds = rounds(&prober, Duration::from_millis(400));
    let targets: Vec<Target> = (1..=40).map(|i| (ip(&format!("10.0.0.{i}")), Method::Ping)).collect();
    let all = futures::future::join_all((0..8).map(|_| rounds.answers(targets.clone()))).await;
    assert_eq!(prober.count(), 40, "eight pages at once: each address asked once");
    assert!(all.iter().all(|a| a.len() == 40 && *a == all[0]));
    assert_eq!(all[0][&(ip("10.0.0.1"), Method::Ping)].outcome, Outcome::Reply { ms: Some(0.612) });
    assert_eq!(all[0][&(ip("10.0.0.2"), Method::Ping)].outcome, Outcome::NoReply);
    assert_eq!(prober.most_at_once.load(Ordering::SeqCst), CONCURRENCY, "a small number at a time");
    // Later, within the time they are kept: nothing more. A new address: only that one.
    rounds.answers(targets.clone()).await;
    let mut more = targets.clone();
    more.push((ip("10.0.0.1"), Method::Tcp(22)));
    assert_eq!(rounds.answers(more).await.len(), 41);
    assert_eq!(prober.count(), 41);
    tokio::time::sleep(Duration::from_millis(450)).await;
    rounds.answers(targets[..2].to_vec()).await;
    assert_eq!(prober.count(), 43, "asked again once old, and only what is still wanted");
    assert!(rounds.answers(Vec::new()).await.is_empty());
    assert_eq!(prober.count(), 43);
}

#[actix_web::test]
async fn a_caller_that_goes_away_does_not_make_the_next_one_ask_again() {
    let prober = FakeProber::new(&answers(), Duration::from_millis(80));
    let rounds = rounds(&prober, Duration::from_secs(60));
    let target = vec![(ip("10.0.0.1"), Method::Ping)];
    assert!(tokio::time::timeout(Duration::from_millis(10), rounds.answers(target.clone())).await.is_err(), "the page closed mid-request");
    let got = rounds.answers(target.clone()).await;
    assert_eq!((prober.count(), &got[&target[0]].outcome), (1, &Outcome::Reply { ms: Some(0.612) }));
}

#[actix_web::test]
async fn a_round_ends_at_its_deadline_and_the_slow_ones_wait_for_the_next() {
    tokio::time::pause();
    let prober = FakeProber::new(&answers(), Duration::from_secs(3600));
    let rounds = rounds(&prober, TTL);
    let targets: Vec<Target> = (1..=20).map(|i| (ip(&format!("10.0.0.{i}")), Method::Ping)).collect();
    let started = tokio::time::Instant::now();
    let got = rounds.answers(targets.clone()).await;
    assert!((ROUND_DEADLINE..ROUND_DEADLINE + Duration::from_millis(100)).contains(&started.elapsed()), "{:?}", started.elapsed());
    assert!(got.len() == 20 && got.values().all(|a| a.outcome == Outcome::Unavailable("not_in_time")));
    rounds.answers(targets).await;
    assert_eq!(prober.count(), CONCURRENCY, "those not reached are not asked before the answers are old either");
}

#[actix_web::test]
async fn more_targets_than_the_limit_are_never_asked() {
    let prober = FakeProber::new(&[], Duration::ZERO);
    let rounds = rounds(&prober, TTL);
    let targets: Vec<Target> = (0..300u32).map(|i| (Ipv4Addr::from(0x0a00_0001 + i), Method::Ping)).collect();
    assert_eq!(rounds.answers(targets).await.len(), MAX_TARGETS);
    assert_eq!(prober.count(), MAX_TARGETS);
}
