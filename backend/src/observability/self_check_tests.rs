use super::*;

fn healthy() -> Facts {
    Facts { disk_free_percent: Some(50.0), audit_ok: true, tmux: Some(Tmux::Reachable), ..Facts::default() }
}

fn subjects(notices: &[Notice]) -> Vec<&str> {
    notices.iter().map(|(s, _)| s.as_str()).collect()
}

#[test]
fn a_failure_notifies_once_reminds_after_the_repeat_and_says_when_it_recovers() {
    let mut checks = SelfChecks::new(ALL.to_vec(), Duration::from_secs(6 * 3600));
    let t0 = Instant::now();
    let min = |m: u64| t0 + Duration::from_secs(m * 60);
    assert!(checks.tick(&healthy(), min(0)).is_empty());
    let full = Facts { disk_free_percent: Some(4.2), ..healthy() };
    let first = checks.tick(&full, min(1));
    assert_eq!(subjects(&first), ["[NabiMan] Self-check failed: disk"]);
    assert!(first[0].1.contains("4.2%"));
    for m in 2..360 {
        assert!(checks.tick(&full, min(m)).is_empty(), "minute {m}: rate-limited");
    }
    assert_eq!(subjects(&checks.tick(&full, min(361))), ["[NabiMan] Self-check still failing: disk"]);
    assert_eq!(subjects(&checks.tick(&healthy(), min(362))), ["[NabiMan] Self-check recovered: disk"]);
    assert!(checks.tick(&healthy(), min(363)).is_empty());
}

#[test]
fn each_check_reads_its_own_fact() {
    let t = Instant::now();
    let fails = |facts: Facts| {
        let mut checks = SelfChecks::new(ALL.to_vec(), Duration::from_secs(3600));
        checks.tick(&healthy(), t);
        subjects(&checks.tick(&facts, t + Duration::from_secs(60))).iter().map(|s| s.rsplit(' ').next().unwrap().to_string()).collect::<Vec<_>>()
    };
    assert_eq!(fails(Facts { audit_failures_total: 1, ..healthy() }), ["audit"]);
    assert_eq!(fails(Facts { audit_ok: false, ..healthy() }), ["audit"]);
    assert_eq!(fails(Facts { backups_overdue: vec!["nightly".into()], ..healthy() }), ["backup"]);
    assert_eq!(fails(Facts { tmux: Some(Tmux::Unreachable), ..healthy() }), ["tmux"]);
    assert_eq!(fails(Facts { tmux: Some(Tmux::NoServer), registered_sessions: 2, ..healthy() }), ["tmux"]);
    assert!(fails(Facts { tmux: Some(Tmux::NoServer), ..healthy() }).is_empty(), "no sessions, no tmux: fine");
    assert!(fails(Facts { child_timeouts_total: 4, ..healthy() }).is_empty());
    assert_eq!(fails(Facts { child_timeouts_total: 5, ..healthy() }), ["timeouts"]);
}

#[test]
fn old_timeouts_fall_out_of_the_window() {
    let mut checks = SelfChecks::new(vec![Check::Timeouts], Duration::from_secs(3600));
    let t0 = Instant::now();
    let at = |m: u64| t0 + Duration::from_secs(m * 60);
    for (m, total) in [(0, 0), (1, 3), (20, 6), (21, 7)] {
        assert!(checks.tick(&Facts { child_timeouts_total: total, ..healthy() }, at(m)).is_empty(), "minute {m}");
    }
    let burst = checks.tick(&Facts { child_timeouts_total: 12, ..healthy() }, at(22));
    assert_eq!(subjects(&burst), ["[NabiMan] Self-check failed: timeouts"]);
}

#[test]
fn checks_are_switched_by_one_setting() {
    let with = |v: &'static str| move |name: &str| (name == "NABIMAN_SELF_CHECKS").then(|| v.to_string());
    assert_eq!(enabled_in(&|_| None), ALL.to_vec());
    assert_eq!(enabled_in(&with("on")), ALL.to_vec());
    assert!(enabled_in(&with("off")).is_empty());
    assert_eq!(enabled_in(&with("disk, Backup")), [Check::Disk, Check::Backup]);
}

#[test]
fn a_real_directory_has_some_free_space() {
    let free = free_percent(&std::env::temp_dir()).unwrap();
    assert!((0.0..=100.0).contains(&free));
    assert!(free_percent(std::path::Path::new("/no/such/dir")).is_none());
}
