use super::client::upstream_err;
use crate::mec::services::ServiceResult;
use crate::models::mec::{ClusterEvent, NodeMetrics, PodPhaseSummary, TenantUsage};
use k8s_openapi::api::core::v1::{Event, Pod};
use kube::api::ListParams;
use kube::core::{ApiResource, DynamicObject, GroupVersionKind};
use kube::{Api, Client};
use std::collections::BTreeMap;

fn metrics_resource(kind: &str, plural: &str) -> ApiResource {
    let gvk = GroupVersionKind::gvk("metrics.k8s.io", "v1beta1", kind);
    ApiResource::from_gvk_with_plural(&gvk, plural)
}

/// Bound list responses so a very large cluster can't blow memory / a single
/// response. 5000 covers realistic MEC clusters without paging; beyond that
/// the tail is dropped (documented cap, not silent truncation of small sets).
fn capped() -> ListParams {
    ListParams::default().limit(5000)
}

/// k8s CPU quantity (e.g. "123456789n", "250m", "2") -> millicores.
fn cpu_millicores(s: &str) -> u64 {
    let s = s.trim();
    if s.is_empty() {
        return 0;
    }
    let (num, div) = if let Some(v) = s.strip_suffix('n') {
        (v, 1_000_000.0)
    } else if let Some(v) = s.strip_suffix('u') {
        (v, 1_000.0)
    } else if let Some(v) = s.strip_suffix('m') {
        (v, 1.0)
    } else {
        (s, 0.001) // whole cores -> *1000
    };
    (num.parse::<f64>().unwrap_or(0.0) / div).round() as u64
}

/// k8s memory quantity (e.g. "263849492Ki", "128Mi", "1Gi") -> bytes.
fn mem_bytes(s: &str) -> u64 {
    let s = s.trim();
    const UNITS: &[(&str, f64)] = &[
        ("Ki", 1024.0),
        ("Mi", 1_048_576.0),
        ("Gi", 1_073_741_824.0),
        ("Ti", 1_099_511_627_776.0),
        ("k", 1e3),
        ("M", 1e6),
        ("G", 1e9),
        ("T", 1e12),
    ];
    for (suf, mult) in UNITS {
        if let Some(v) = s.strip_suffix(suf) {
            return (v.trim().parse::<f64>().unwrap_or(0.0) * mult) as u64;
        }
    }
    s.parse::<u64>().unwrap_or(0)
}

fn pct(used: u64, cap: u64) -> f32 {
    if cap == 0 {
        0.0
    } else {
        ((used as f64 / cap as f64) * 100.0) as f32
    }
}

pub async fn node_metrics(client: &Client) -> ServiceResult<Vec<NodeMetrics>> {
    // Capacity from the node objects, live usage from metrics-server.
    let nodes = super::nodes::list(client).await?;
    let mut cap: BTreeMap<String, (u64, u64)> = BTreeMap::new();
    for n in &nodes {
        cap.insert(
            n.name.clone(),
            (cpu_millicores(&n.capacity.cpu), mem_bytes(&n.capacity.memory)),
        );
    }

    let api: Api<DynamicObject> = Api::all_with(client.clone(), &metrics_resource("NodeMetrics", "nodes"));
    let list = api.list(&capped()).await.map_err(upstream_err)?;
    let mut out = Vec::new();
    for item in list {
        let name = item.metadata.name.clone().unwrap_or_default();
        let usage = item.data.get("usage");
        let cpu = cpu_millicores(usage.and_then(|u| u.get("cpu")).and_then(|v| v.as_str()).unwrap_or(""));
        let mem = mem_bytes(usage.and_then(|u| u.get("memory")).and_then(|v| v.as_str()).unwrap_or(""));
        let (cpu_cap, mem_cap) = cap.get(&name).copied().unwrap_or((0, 0));
        out.push(NodeMetrics {
            name,
            status: String::new(), // merged from inventory in build_live
            cpu_usage_millicores: cpu,
            cpu_capacity_millicores: cpu_cap,
            memory_usage_bytes: mem,
            memory_capacity_bytes: mem_cap,
            cpu_usage_percent: pct(cpu, cpu_cap),
            memory_usage_percent: pct(mem, mem_cap),
            gpu_usage_percent: None,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

pub async fn tenant_usage(client: &Client) -> ServiceResult<Vec<TenantUsage>> {
    let api: Api<DynamicObject> = Api::all_with(client.clone(), &metrics_resource("PodMetrics", "pods"));
    let list = api.list(&capped()).await.map_err(upstream_err)?;
    let mut agg: BTreeMap<String, (u64, u64, u32)> = BTreeMap::new();
    for item in list {
        let ns = item.metadata.namespace.clone().unwrap_or_default();
        let (mut cpu, mut mem) = (0u64, 0u64);
        if let Some(conts) = item.data.get("containers").and_then(|c| c.as_array()) {
            for c in conts {
                let u = c.get("usage");
                cpu += cpu_millicores(u.and_then(|x| x.get("cpu")).and_then(|v| v.as_str()).unwrap_or(""));
                mem += mem_bytes(u.and_then(|x| x.get("memory")).and_then(|v| v.as_str()).unwrap_or(""));
            }
        }
        let e = agg.entry(ns).or_insert((0, 0, 0));
        e.0 += cpu;
        e.1 += mem;
        e.2 += 1;
    }
    let mut out: Vec<TenantUsage> = agg
        .into_iter()
        .map(|(namespace, (cpu_millicores, memory_bytes, pods))| TenantUsage {
            namespace,
            cpu_millicores,
            memory_bytes,
            pods,
        })
        .collect();
    out.sort_by(|a, b| b.cpu_millicores.cmp(&a.cpu_millicores));
    Ok(out)
}

pub async fn pod_phase_summary(client: &Client) -> ServiceResult<PodPhaseSummary> {
    let api: Api<Pod> = Api::all(client.clone());
    let list = api.list(&capped()).await.map_err(upstream_err)?;
    let mut s = PodPhaseSummary::default();
    for p in list {
        s.total += 1;
        match p.status.as_ref().and_then(|st| st.phase.as_deref()) {
            Some("Running") => s.running += 1,
            Some("Pending") => s.pending += 1,
            Some("Failed") => s.failed += 1,
            Some("Succeeded") => s.succeeded += 1,
            _ => s.unknown += 1,
        }
    }
    Ok(s)
}

pub async fn list_events(client: &Client, limit: u32) -> ServiceResult<Vec<ClusterEvent>> {
    let api: Api<Event> = Api::all(client.clone());
    let list = api.list(&capped()).await.map_err(upstream_err)?;
    let mut events: Vec<(chrono::DateTime<chrono::Utc>, ClusterEvent)> = list
        .into_iter()
        .map(|e| {
            let when = e
                .last_timestamp
                .as_ref()
                .map(|t| t.0)
                .or_else(|| e.event_time.as_ref().map(|t| t.0))
                .unwrap_or_else(chrono::Utc::now);
            let ev = ClusterEvent {
                last_time: Some(when.to_rfc3339()),
                event_type: e.type_.unwrap_or_default(),
                reason: e.reason.unwrap_or_default(),
                kind: e.involved_object.kind.unwrap_or_default(),
                name: e.involved_object.name.unwrap_or_default(),
                namespace: e.involved_object.namespace,
                message: e.message.unwrap_or_default(),
                count: e.count.unwrap_or(1).max(0) as u32,
            };
            (when, ev)
        })
        .collect();
    events.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(events.into_iter().take(limit as usize).map(|(_, e)| e).collect())
}
