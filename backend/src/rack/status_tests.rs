use super::layout::{parse, Layout};
use super::layout_tests::SAMPLE;
use super::prometheus::{parse_answer, Metric, Samples, QUERIES};
use super::status::*;

fn vector(series: &[(&[(&str, &str)], &str)]) -> String {
    let items: Vec<serde_json::Value> = series
        .iter()
        .map(|(labels, value)| {
            let metric: serde_json::Map<String, serde_json::Value> = labels.iter().map(|(k, v)| (k.to_string(), serde_json::json!(v))).collect();
            serde_json::json!({"metric": metric, "value": [1795606095.3, value]})
        })
        .collect();
    serde_json::json!({"status": "success", "data": {"resultType": "vector", "result": items}}).to_string()
}

/// What a healthy cluster answers to each query, for the nodes of `SAMPLE`:
/// cp01 (no GPU), wn02 (two A40), wn03 (one GH200 as two MIG slices, which
/// report no utilisation), cp02 (two T4).
pub(super) fn canned(metric: Metric) -> String {
    let gpu = |values: [&str; 6]| {
        vector(&[
            (&[("Hostname", "mec-wn02"), ("gpu", "0"), ("modelName", "NVIDIA A40")], values[0]),
            (&[("Hostname", "mec-wn02"), ("gpu", "1"), ("modelName", "NVIDIA A40")], values[1]),
            (&[("Hostname", "mec-wn03"), ("gpu", "0"), ("GPU_I_ID", "1"), ("GPU_I_PROFILE", "3g.47gb"), ("modelName", "NVIDIA GH200 480GB")], values[2]),
            (&[("Hostname", "mec-wn03"), ("gpu", "0"), ("GPU_I_ID", "2"), ("GPU_I_PROFILE", "3g.47gb"), ("modelName", "NVIDIA GH200 480GB")], values[3]),
            (&[("Hostname", "mec-cp02"), ("gpu", "0"), ("modelName", "Tesla T4")], values[4]),
            (&[("Hostname", "mec-cp02"), ("gpu", "1"), ("modelName", "Tesla T4")], values[5]),
            (&[("Hostname", "elsewhere"), ("gpu", "0"), ("modelName", "Other")], "99"),
        ])
    };
    let node = |values: [&str; 4]| {
        vector(&[
            (&[("instance", "10.0.0.201:9796")], values[0]),
            (&[("instance", "10.0.0.202:9796")], values[1]),
            (&[("instance", "10.0.0.204:9796")], values[2]),
            (&[("instance", "10.0.0.199:9796")], values[3]),
            (&[("instance", "10.9.9.9:9796")], "99"),
        ])
    };
    match metric {
        Metric::Ready => vector(&[(&[("node", "mec-cp01")], "1"), (&[("node", "mec-wn02")], "1"), (&[("node", "mec-wn03")], "1"), (&[("node", "mec-cp02")], "1")]),
        Metric::Cpu => node(["12.34", "6.19", "0.89", "20"]),
        Metric::Memory => node(["30", "41.26", "27", "11"]),
        Metric::Disk => node(["53.6", "26.7", "20.8", "60.3"]),
        Metric::BoardTemp => node(["55", "54", "70", "62"]),
        Metric::GpuTemp => gpu(["41", "43", "32", "32", "44", "45"]),
        Metric::GpuPower => gpu(["70.5", "117.652", "125.274", "124.952", "27", "28"]),
        Metric::GpuMemUsed => gpu(["601", "0", "38689", "41980", "5", "7"]),
        Metric::GpuMemFree => gpu(["44886", "45487", "8926", "5635", "15000", "15000"]),
        // MIG slices report no utilisation.
        Metric::GpuUtil => vector(&[
            (&[("Hostname", "mec-wn02"), ("gpu", "0"), ("modelName", "NVIDIA A40")], "35"),
            (&[("Hostname", "mec-wn02"), ("gpu", "1"), ("modelName", "NVIDIA A40")], "0"),
            (&[("Hostname", "mec-cp02"), ("gpu", "0"), ("modelName", "Tesla T4")], "3"),
            (&[("Hostname", "mec-cp02"), ("gpu", "1"), ("modelName", "Tesla T4")], "0"),
        ]),
    }
}

fn samples_with(change: &dyn Fn(Metric) -> Option<String>) -> Samples {
    let mut samples = Samples::default();
    for (metric, _) in QUERIES {
        let body = change(metric).unwrap_or_else(|| canned(metric));
        samples.by_metric.insert(metric, parse_answer(body.as_bytes()).unwrap());
    }
    samples
}

fn layout() -> Layout {
    parse(SAMPLE.as_bytes()).unwrap()
}

fn status_with(change: &dyn Fn(Metric) -> Option<String>) -> Status {
    assemble(&layout(), Source::Collected(&samples_with(change)), "2026-11-25T12:00:00Z".into())
}

#[test]
fn a_healthy_cluster_reads_ok_with_every_value_in_place() {
    let s = status_with(&|_| None);
    assert_eq!((s.source, s.message.as_deref(), s.layout_version.as_str(), s.collected_at.as_str()), ("ok", None, "t1", "2026-11-25T12:00:00Z"));
    let cp01 = &s.nodes["mec-cp01"];
    assert_eq!((cp01.level, cp01.ready, cp01.cpu_pct, cp01.mem_pct, cp01.disk_pct, cp01.temp_c, cp01.gpus.len()), (Level::Ok, Some(true), Some(12.3), Some(30.0), Some(53.6), Some(55.0), 0));
    let wn02 = &s.nodes["mec-wn02"];
    assert_eq!(wn02.gpus.len(), 2);
    let a40 = &wn02.gpus[0];
    assert_eq!((a40.index, a40.mig_id, a40.model.as_deref(), a40.util_pct, a40.temp_c, a40.power_w, a40.mem_used_mib, a40.mem_total_mib), (0, None, Some("NVIDIA A40"), Some(35.0), Some(41.0), Some(70.5), Some(601.0), Some(45487.0)));
    assert_eq!(s.summary, Summary { nodes_total: 4, nodes_ready: 4, gpus: 5, warnings: 0, down: 0, max_temp_c: Some(70.0) });
    assert_eq!(s.thresholds, Thresholds { temp_c: 80.0, memory_pct: 90.0, disk_pct: 90.0 });
    assert!(!s.nodes.contains_key("elsewhere"), "only the layout's nodes are reported");
}

#[test]
fn a_gpu_cut_into_mig_slices_is_two_entries_and_one_gpu() {
    let s = status_with(&|_| None);
    let gpus = &s.nodes["mec-wn03"].gpus;
    assert_eq!(gpus.len(), 2);
    assert_eq!((gpus[0].index, gpus[0].mig_id, gpus[0].mig_profile.as_deref(), gpus[0].util_pct), (0, Some(1), Some("3g.47gb"), None));
    assert_eq!((gpus[1].mig_id, gpus[1].temp_c, gpus[1].power_w, gpus[1].mem_used_mib, gpus[1].mem_total_mib), (Some(2), Some(32.0), Some(125.0), Some(41980.0), Some(47615.0)));
    assert_eq!(s.nodes["mec-wn03"].level, Level::Ok);
    // 2 (A40) + 1 (GH200, two slices) + 2 (T4)
    assert_eq!(s.summary.gpus, 5);
}

#[test]
fn devices_take_the_worst_of_their_nodes_and_none_without_one() {
    let s = status_with(&|m| (m == Metric::Memory).then(|| vector(&[(&[("instance", "10.0.0.202:9796")], "93.5"), (&[("instance", "10.0.0.201:9796")], "30")])));
    assert_eq!((s.nodes["mec-cp01"].level, s.nodes["mec-wn02"].level), (Level::Ok, Level::Warn));
    assert_eq!(s.nodes["mec-wn02"].reasons, ["memory"]);
    for id in ["lenovo-1", "lenovo-2"] {
        assert_eq!(s.devices[id], DeviceStatus { level: Level::Warn, nodes: vec!["mec-cp01".into(), "mec-wn02".into()] }, "{id}");
    }
    assert_eq!(s.devices["sw"], DeviceStatus { level: Level::None, nodes: vec![] }, "an address alone is not collected");
    assert_eq!(s.devices["odd"].level, Level::None);
    assert_eq!(s.devices["gh-wn03"].nodes, ["mec-wn03"]);
    // Each node counts once though two machines show it.
    assert_eq!((s.summary.nodes_total, s.summary.nodes_ready, s.summary.warnings), (4, 4, 1));
}

#[test]
fn the_warning_thresholds() {
    let at = |metric: Metric, value: &'static str| status_with(&move |m| (m == metric).then(|| vector(&[(&[("instance", "10.0.0.201:9796")], value)])));
    for (metric, below, on, reason) in [(Metric::BoardTemp, "79.9", "80", "board_temp"), (Metric::Memory, "89.9", "90", "memory"), (Metric::Disk, "89.94", "90", "disk")] {
        assert_eq!(at(metric, below).nodes["mec-cp01"].level, Level::Ok, "{metric:?} {below}");
        let warned = at(metric, on);
        assert_eq!((warned.nodes["mec-cp01"].level, warned.nodes["mec-cp01"].reasons.as_slice()), (Level::Warn, [reason].as_slice()), "{metric:?} {on}");
    }
    let hot_gpu = status_with(&|m| (m == Metric::GpuTemp).then(|| vector(&[(&[("Hostname", "mec-cp02"), ("gpu", "1")], "84")])));
    assert_eq!((hot_gpu.nodes["mec-cp02"].level, hot_gpu.nodes["mec-cp02"].reasons.as_slice()), (Level::Warn, ["gpu_temp"].as_slice()));
    assert_eq!((hot_gpu.devices["t4"].level, hot_gpu.summary.max_temp_c, hot_gpu.summary.warnings), (Level::Warn, Some(84.0), 1));
    // CPU alone never warns.
    assert_eq!(at(Metric::Cpu, "100").nodes["mec-cp01"].level, Level::Ok);
}

#[test]
fn not_ready_is_down_and_outranks_a_warning() {
    let s = status_with(&|m| match m {
        Metric::Ready => Some(vector(&[(&[("node", "mec-cp01")], "0"), (&[("node", "mec-wn02")], "1"), (&[("node", "mec-wn03")], "1"), (&[("node", "mec-cp02")], "1")])),
        Metric::BoardTemp => Some(vector(&[(&[("instance", "10.0.0.201:9796")], "91"), (&[("instance", "10.0.0.202:9796")], "85")])),
        _ => None,
    });
    assert_eq!((s.nodes["mec-cp01"].level, s.nodes["mec-cp01"].ready), (Level::Down, Some(false)));
    assert_eq!(s.nodes["mec-wn02"].level, Level::Warn);
    assert_eq!(s.devices["lenovo-1"].level, Level::Down, "the worst candidate decides");
    assert_eq!((s.summary.nodes_ready, s.summary.down, s.summary.warnings), (3, 1, 1));
    assert!(Level::Down > Level::Warn && Level::Warn > Level::Unknown && Level::Unknown > Level::Ok && Level::Ok > Level::None);
}

#[test]
fn a_node_prometheus_does_not_know_is_unknown() {
    let s = status_with(&|m| (m == Metric::Ready).then(|| vector(&[(&[("node", "mec-cp01")], "1")])));
    assert_eq!((s.nodes["mec-cp01"].level, s.nodes["mec-wn02"].level, s.nodes["mec-wn02"].ready), (Level::Ok, Level::Unknown, None));
    assert_eq!(s.nodes["mec-wn02"].cpu_pct, Some(6.2), "what did come back is still shown");
    assert_eq!(s.devices["lenovo-1"].level, Level::Unknown, "unknown is worse than ok");
    let empty = status_with(&|_| Some(vector(&[])));
    assert!(empty.nodes.values().all(|n| n.level == Level::Unknown && n.cpu_pct.is_none() && n.gpus.is_empty()));
    assert_eq!(empty.summary, Summary { nodes_total: 4, ..Summary::default() });
}

#[test]
fn no_prometheus_means_unknown_everywhere_with_the_reason() {
    let down = assemble(&layout(), Source::Unreachable("cannot connect"), "t".into());
    assert_eq!((down.source, down.message.as_deref()), ("unreachable", Some("cannot connect")));
    assert!(down.nodes.len() == 4 && down.nodes.values().all(|n| n.level == Level::Unknown));
    assert_eq!((down.devices["lenovo-1"].level, down.devices["t4"].level, down.devices["sw"].level), (Level::Unknown, Level::Unknown, Level::None));
    let off = assemble(&layout(), Source::NotConfigured, "t".into());
    assert_eq!((off.source, off.message, off.devices["t4"].level), ("not_configured", None, Level::Unknown));
    let mut partial = samples_with(&|_| None);
    partial.by_metric.remove(&Metric::Ready);
    partial.failed.push("Ready".into());
    let s = assemble(&layout(), Source::Collected(&partial), "t".into());
    assert_eq!((s.source, s.message.as_deref(), s.nodes["mec-cp01"].level), ("partial", Some("no answer for: Ready"), Level::Unknown));
}

#[test]
fn answers_that_are_not_what_prometheus_sends_are_refused() {
    assert!(parse_answer(b"<html>502</html>").is_err());
    assert!(parse_answer(br#"{"status":"error","errorType":"bad_data","error":"parse error"}"#).unwrap_err().contains("parse error"));
    let odd = br#"{"status":"success","data":{"resultType":"vector","result":[{"metric":{"instance":"a:1"},"value":[1,"NaN"]},{"metric":{},"value":[1,"+Inf"]},{"metric":{"instance":"b:1"},"value":[1,"7"]}]}}"#;
    let kept = parse_answer(odd).unwrap();
    assert_eq!((kept.len(), kept[0].value), (1, 7.0), "values that are not finite numbers are left out");
    assert!(parse_answer(br#"{"status":"success","data":{"result":[]}}"#).unwrap().is_empty());
}

#[test]
fn the_queries_are_fixed_text_and_never_name_a_node() {
    let layout = layout();
    for (_, query) in QUERIES {
        assert!(!query.contains("mec-") && !query.contains("10.0.0.") && !query.contains('<'), "{query}");
        for node in &layout.nodes {
            assert!(!query.contains(&node.name));
        }
    }
    assert_eq!(QUERIES.len(), 10);
}
