//! GPU utilization from an NVIDIA dcgm-exporter.
//!
//! dcgm-exporter publishes a stable Prometheus metric, `DCGM_FI_DEV_GPU_UTIL`
//! (percent, per GPU), each sample labelled with the `Hostname` it runs on.
//! We scrape it over HTTP and average per node, then enrich NodeMetrics so the
//! NOC wall can show real GPU load. The endpoint is opt-in via
//! `NABIMAN_MEC_DCGM_URL`; when unset we return an empty map and the UI keeps
//! its honest "data source not connected" placeholder — no fabricated numbers.

use std::collections::HashMap;
use std::time::Duration;

/// Average GPU utilization (%) per node, keyed by hostname. Empty when the
/// exporter isn't configured or is unreachable (degrade, never fail the wall).
pub async fn node_gpu_util() -> HashMap<String, f32> {
    let url = match std::env::var("NABIMAN_MEC_DCGM_URL") {
        Ok(u) if !u.is_empty() => u,
        _ => return HashMap::new(),
    };
    let client = match reqwest::Client::builder().timeout(Duration::from_secs(5)).build() {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };
    match client.get(&url).send().await {
        Ok(resp) => match resp.text().await {
            Ok(body) => parse_dcgm(&body),
            Err(_) => HashMap::new(),
        },
        Err(_) => HashMap::new(),
    }
}

/// Parse dcgm-exporter's Prometheus text into avg GPU util% per Hostname.
pub fn parse_dcgm(text: &str) -> HashMap<String, f32> {
    let mut sums: HashMap<String, (f32, u32)> = HashMap::new();
    for line in text.lines() {
        if !line.starts_with("DCGM_FI_DEV_GPU_UTIL{") {
            continue;
        }
        let host = match label_value(line, "Hostname") {
            Some(h) => h,
            None => continue,
        };
        let value = match line.rsplit(|c: char| c == '}' || c.is_whitespace()).find(|t| !t.is_empty()) {
            Some(v) => match v.parse::<f32>() {
                Ok(n) => n,
                Err(_) => continue,
            },
            None => continue,
        };
        let e = sums.entry(host).or_insert((0.0, 0));
        e.0 += value;
        e.1 += 1;
    }
    sums.into_iter()
        .filter(|(_, (_, n))| *n > 0)
        .map(|(h, (sum, n))| (h, sum / n as f32))
        .collect()
}

/// Extract `key="value"` from a Prometheus label set.
fn label_value(line: &str, key: &str) -> Option<String> {
    let needle = format!("{}=\"", key);
    let start = line.find(&needle)? + needle.len();
    let end = line[start..].find('"')? + start;
    Some(line[start..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"# HELP DCGM_FI_DEV_GPU_UTIL GPU utilization (in %).
# TYPE DCGM_FI_DEV_GPU_UTIL gauge
DCGM_FI_DEV_GPU_UTIL{gpu="0",UUID="GPU-a",device="nvidia0",modelName="A100",Hostname="mec-gpu01"} 40
DCGM_FI_DEV_GPU_UTIL{gpu="1",UUID="GPU-b",device="nvidia1",modelName="A100",Hostname="mec-gpu01"} 60
DCGM_FI_DEV_GPU_UTIL{gpu="0",UUID="GPU-c",device="nvidia0",modelName="A100",Hostname="mec-gpu02"} 90
DCGM_FI_DEV_DEC_UTIL{gpu="0",Hostname="mec-gpu01"} 5
"#;

    #[test]
    fn averages_util_per_host() {
        let m = parse_dcgm(SAMPLE);
        assert_eq!(m.get("mec-gpu01"), Some(&50.0)); // (40+60)/2, ignores DEC_UTIL
        assert_eq!(m.get("mec-gpu02"), Some(&90.0));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn empty_input_is_empty_map() {
        assert!(parse_dcgm("").is_empty());
        assert!(parse_dcgm("# just comments\n").is_empty());
    }

    #[test]
    fn extracts_label_value() {
        assert_eq!(label_value(r#"x{Hostname="n1",gpu="0"} 5"#, "Hostname"), Some("n1".into()));
        assert_eq!(label_value("x{gpu=\"0\"} 5", "Hostname"), None);
    }
}
