//! The metrics behind the rack page: a fixed, small set of instant queries,
//! each answering for every node at once. Nothing from the layout file ever
//! goes into a query; names and addresses from the file are only compared
//! with the labels that come back (see `status`).
//!
//! Same client and manner as `mec::services::gpu_dcgm` (reqwest, a few
//! seconds' timeout, degrade instead of failing), with a cap on how much of
//! an answer is read.

use std::collections::HashMap;
use std::time::Duration;

use serde::Deserialize;

/// What each query is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Metric {
    Ready,
    Cpu,
    Memory,
    Disk,
    BoardTemp,
    GpuUtil,
    GpuTemp,
    GpuPower,
    GpuMemUsed,
    GpuMemFree,
}

/// Every query the rack page makes. Node metrics are per `instance`
/// (node_exporter, `<ip>:<port>`), GPU metrics per DCGM `Hostname` and GPU.
pub(crate) const QUERIES: [(Metric, &str); 10] = [
    (Metric::Ready, r#"max by (node) (kube_node_status_condition{condition="Ready",status="true"})"#),
    (Metric::Cpu, r#"100 * (1 - avg by (instance) (rate(node_cpu_seconds_total{mode="idle"}[5m])))"#),
    (Metric::Memory, "100 * (1 - max by (instance) (node_memory_MemAvailable_bytes) / max by (instance) (node_memory_MemTotal_bytes))"),
    (Metric::Disk, r#"100 * (1 - max by (instance) (node_filesystem_avail_bytes{mountpoint="/"}) / max by (instance) (node_filesystem_size_bytes{mountpoint="/"}))"#),
    (Metric::BoardTemp, "max by (instance) (node_hwmon_temp_celsius)"),
    (Metric::GpuUtil, "max by (Hostname, gpu, GPU_I_ID, GPU_I_PROFILE, modelName) (DCGM_FI_DEV_GPU_UTIL)"),
    (Metric::GpuTemp, "max by (Hostname, gpu, GPU_I_ID, GPU_I_PROFILE, modelName) (DCGM_FI_DEV_GPU_TEMP)"),
    (Metric::GpuPower, "max by (Hostname, gpu, GPU_I_ID, GPU_I_PROFILE, modelName) (DCGM_FI_DEV_POWER_USAGE)"),
    (Metric::GpuMemUsed, "max by (Hostname, gpu, GPU_I_ID, GPU_I_PROFILE, modelName) (DCGM_FI_DEV_FB_USED)"),
    (Metric::GpuMemFree, "max by (Hostname, gpu, GPU_I_ID, GPU_I_PROFILE, modelName) (DCGM_FI_DEV_FB_FREE)"),
];

/// One query's whole wait, and all of them together.
const QUERY_TIMEOUT: Duration = Duration::from_secs(4);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
pub(crate) const OVERALL_TIMEOUT: Duration = Duration::from_secs(6);
/// More than any honest answer to these queries; the rest is not read.
const MAX_ANSWER_BYTES: usize = 2 * 1024 * 1024;
const MAX_SERIES: usize = 5000;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Sample {
    pub labels: HashMap<String, String>,
    pub value: f64,
}

/// What Prometheus said: the series of each query that was answered.
#[derive(Clone, Debug, Default)]
pub(crate) struct Samples {
    pub by_metric: HashMap<Metric, Vec<Sample>>,
    /// Queries that were not answered, when some were.
    pub failed: Vec<String>,
}

impl Samples {
    pub(crate) fn of(&self, metric: Metric) -> &[Sample] {
        self.by_metric.get(&metric).map(Vec::as_slice).unwrap_or_default()
    }
}

/// `rack_prometheus_url` as a base to query: http(s), a host, no sign-in
/// in it, no query. A path is kept (Prometheus behind a prefix).
pub(crate) fn base_url(text: &str) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(text.trim()).map_err(|e| format!("not a URL: {e}"))?;
    let plain = matches!(url.scheme(), "http" | "https")
        && url.host_str().is_some_and(|h| !h.is_empty())
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none();
    if !plain {
        return Err("must be http(s)://host[:port][/path] with no sign-in, query or fragment".into());
    }
    Ok(url)
}

fn query_url(base: &reqwest::Url) -> reqwest::Url {
    let mut url = base.clone();
    let path = format!("{}/api/v1/query", base.path().trim_end_matches('/'));
    url.set_path(&path);
    url
}

pub(crate) fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(QUERY_TIMEOUT)
        .connect_timeout(CONNECT_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| format!("HTTP client: {e}"))
}

#[derive(Deserialize)]
struct Answer {
    status: String,
    #[serde(default)]
    data: Option<AnswerData>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Deserialize)]
struct AnswerData {
    #[serde(default)]
    result: Vec<Series>,
}

#[derive(Deserialize)]
struct Series {
    #[serde(default)]
    metric: HashMap<String, String>,
    /// `[time, "value"]`.
    value: (f64, String),
}

/// The series in one instant-query answer; values that are not finite
/// numbers (NaN from a division by zero) are left out.
pub(crate) fn parse_answer(body: &[u8]) -> Result<Vec<Sample>, String> {
    let answer: Answer = serde_json::from_slice(body).map_err(|e| format!("not a Prometheus answer: {e}"))?;
    if answer.status != "success" {
        let said: String = answer.error.unwrap_or_default().chars().take(200).collect();
        return Err(format!("Prometheus refused the query: {said}"));
    }
    let series = answer.data.map(|d| d.result).unwrap_or_default();
    if series.len() > MAX_SERIES {
        return Err(format!("more than {MAX_SERIES} series in one answer"));
    }
    Ok(series
        .into_iter()
        .filter_map(|s| s.value.1.parse::<f64>().ok().filter(|v| v.is_finite()).map(|value| Sample { labels: s.metric, value }))
        .collect())
}

async fn ask(client: &reqwest::Client, url: reqwest::Url, query: &str) -> Result<Vec<Sample>, String> {
    let mut response = client.get(url).query(&[("query", query)]).send().await.map_err(|e| {
        if e.is_timeout() { "timed out".to_string() } else if e.is_connect() { "cannot connect".to_string() } else { "request failed".to_string() }
    })?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status().as_u16()));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| "the answer broke off".to_string())? {
        if body.len() + chunk.len() > MAX_ANSWER_BYTES {
            return Err("the answer is too large".into());
        }
        body.extend_from_slice(&chunk);
    }
    parse_answer(&body)
}

/// Ask every query at once. `Err`: Prometheus gave no answer at all (the
/// reason, for the page). Some answered: those, and which did not.
pub(crate) async fn collect(client: &reqwest::Client, base: &reqwest::Url) -> Result<Samples, String> {
    let url = query_url(base);
    let all = futures::future::join_all(QUERIES.iter().map(|(_, query)| ask(client, url.clone(), query)));
    let answers = match tokio::time::timeout(OVERALL_TIMEOUT, all).await {
        Ok(answers) => answers,
        Err(_) => return Err("timed out".into()),
    };
    let mut samples = Samples::default();
    let mut first_error = None;
    for ((metric, _), answer) in QUERIES.iter().zip(answers) {
        match answer {
            Ok(series) => {
                samples.by_metric.insert(*metric, series);
            }
            Err(e) => {
                samples.failed.push(format!("{metric:?}"));
                first_error.get_or_insert(e);
            }
        }
    }
    match first_error {
        Some(e) if samples.by_metric.is_empty() => Err(e),
        _ => Ok(samples),
    }
}
