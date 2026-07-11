use super::util::{from_service_error, ok_response};
use crate::mec::services::ServiceBundle;
use crate::mec::MecState;
use crate::models::mec::{ClusterHealth, ClusterUsage, LiveSnapshot, NodeMetrics, NodeStatus};
use actix_web::{web, HttpResponse};
use serde::Serialize;
use std::collections::HashMap;

pub async fn summary(state: web::Data<MecState>) -> HttpResponse {
    match build_summary(&state).await {
        Ok(s) => ok_response(s),
        Err(e) => from_service_error(e),
    }
}

pub async fn activity(state: web::Data<MecState>) -> HttpResponse {
    match state.audit_store.count_recent(10) {
        Ok(logs) => super::util::list_response(logs),
        Err(e) => from_service_error(e.into()),
    }
}

pub async fn live(state: web::Data<MecState>) -> HttpResponse {
    match build_live(&state.services).await {
        Ok(s) => ok_response(s),
        Err(e) => from_service_error(e),
    }
}

pub async fn build_live(services: &ServiceBundle) -> crate::mec::services::ServiceResult<LiveSnapshot> {
    // Node inventory (authoritative status + GPU slots) merged with metrics-server
    // usage, so NotReady/DOWN nodes — which metrics-server omits — still appear.
    let inventory = services.kube.list_nodes().await?;
    let metrics = services.kube.node_metrics().await?;
    let pods = services.kube.pod_phase_summary().await?;
    let mut tenants = services.kube.tenant_usage().await?;
    tenants.truncate(12);
    let events = services.kube.list_events(25).await?;

    // Real GPU utilization%, if a dcgm-exporter is wired up (else empty → the
    // gpu_usage_percent stays None and the UI shows its honest placeholder).
    let gpu_util = crate::mec::services::gpu_dcgm::node_gpu_util().await;

    let mut by_name: HashMap<String, NodeMetrics> =
        metrics.into_iter().map(|m| (m.name.clone(), m)).collect();
    let nodes: Vec<NodeMetrics> = inventory
        .iter()
        .map(|inv| {
            let status = match inv.status {
                NodeStatus::Ready => "Ready",
                NodeStatus::NotReady => "NotReady",
                NodeStatus::Unknown => "Unknown",
            };
            let mut nm = by_name.remove(&inv.name).unwrap_or_else(|| NodeMetrics {
                name: inv.name.clone(),
                status: String::new(),
                cpu_usage_millicores: 0,
                cpu_capacity_millicores: 0,
                memory_usage_bytes: 0,
                memory_capacity_bytes: 0,
                cpu_usage_percent: 0.0,
                memory_usage_percent: 0.0,
                gpu_usage_percent: None,
            });
            nm.status = status.to_string();
            if let Some(util) = gpu_util.get(&inv.name) {
                nm.gpu_usage_percent = Some(*util);
            }
            nm
        })
        .collect();

    let health = ClusterHealth {
        nodes_total: inventory.len() as u32,
        nodes_ready: inventory
            .iter()
            .filter(|n| matches!(n.status, NodeStatus::Ready))
            .count() as u32,
        gpu_total_slots: inventory
            .iter()
            .filter_map(|n| n.gpu_info.as_ref().map(|g| g.total_slots))
            .sum(),
        gpu_allocated_slots: inventory
            .iter()
            .filter(|n| n.current_tenant.is_some())
            .filter_map(|n| n.gpu_info.as_ref().map(|g| g.total_slots))
            .sum(),
        gpu_available_slots: 0,
        kubeconfig_days: crate::mec::services::kubeconfig_expiry::days_until_expiry_from_env(),
    };
    let health = ClusterHealth {
        gpu_available_slots: health.gpu_total_slots.saturating_sub(health.gpu_allocated_slots),
        ..health
    };

    let mut cluster = ClusterUsage {
        node_count: nodes.len() as u32,
        ..Default::default()
    };
    for n in &nodes {
        cluster.cpu_used_millicores += n.cpu_usage_millicores;
        cluster.cpu_capacity_millicores += n.cpu_capacity_millicores;
        cluster.memory_used_bytes += n.memory_usage_bytes;
        cluster.memory_capacity_bytes += n.memory_capacity_bytes;
    }
    let pct = |u: u64, c: u64| if c == 0 { 0.0 } else { (u as f64 / c as f64 * 100.0) as f32 };
    cluster.cpu_percent = pct(cluster.cpu_used_millicores, cluster.cpu_capacity_millicores);
    cluster.memory_percent = pct(cluster.memory_used_bytes, cluster.memory_capacity_bytes);

    // VPN sessions are best-effort: an AXGATE outage must not sink the whole
    // wall, so a failure degrades to an empty list (honest placeholder).
    let vpn_sessions = services.axgate.list_vpn_sessions().await.unwrap_or_default();

    Ok(LiveSnapshot { cluster, health, nodes, pods, tenants, events, vpn_sessions })
}

#[derive(Serialize)]
pub struct DashboardSummary {
    pub tenants: TenantStats,
    pub nodes: NodeStats,
    pub gpu: GpuStats,
    pub network: NetworkStats,
    pub firewall: FirewallStats,
}

#[derive(Serialize)]
pub struct TenantStats {
    pub active: u32,
    pub total: u32,
}

#[derive(Serialize)]
pub struct NodeStats {
    pub ready: u32,
    pub total: u32,
}

#[derive(Serialize)]
pub struct GpuStats {
    pub total_slots: u32,
    pub allocated_slots: u32,
    pub available_slots: u32,
}

#[derive(Serialize)]
pub struct NetworkStats {
    pub lb_services: u32,
    pub public_ips_assigned: u32,
}

#[derive(Serialize)]
pub struct FirewallStats {
    pub nat_rules: u32,
    pub security_policies: u32,
}

async fn build_summary(
    state: &MecState,
) -> crate::mec::services::ServiceResult<DashboardSummary> {
    let nodes = state.services.kube.list_nodes().await?;
    let total_nodes = nodes.len() as u32;
    let ready = nodes
        .iter()
        .filter(|n| matches!(n.status, crate::models::mec::NodeStatus::Ready))
        .count() as u32;

    let gpu_slots: u32 = nodes
        .iter()
        .filter_map(|n| n.gpu_info.as_ref().map(|g| g.total_slots))
        .sum();
    let allocated_slots: u32 = nodes
        .iter()
        .filter(|n| n.current_tenant.is_some())
        .filter_map(|n| n.gpu_info.as_ref().map(|g| g.total_slots))
        .sum();

    let lb = state.services.kube.list_lb_services(None).await?;
    let pub_ips = state.services.axgate.list_public_ips().await?;
    let nat = state.services.axgate.list_nat_rules().await?;
    let pols = state.services.axgate.list_security_policies().await?;
    let tenants = state.tenants.list().unwrap_or_default();
    let active = tenants
        .iter()
        .filter(|t| matches!(t.status, crate::models::mec::TenantStatus::Active))
        .count() as u32;

    Ok(DashboardSummary {
        tenants: TenantStats {
            active,
            total: tenants.len() as u32,
        },
        nodes: NodeStats {
            ready,
            total: total_nodes,
        },
        gpu: GpuStats {
            total_slots: gpu_slots,
            allocated_slots,
            available_slots: gpu_slots.saturating_sub(allocated_slots),
        },
        network: NetworkStats {
            lb_services: lb.len() as u32,
            public_ips_assigned: pub_ips
                .iter()
                .filter(|p| {
                    matches!(
                        p.status,
                        crate::models::mec::PublicIpStatus::Assigned
                            | crate::models::mec::PublicIpStatus::SystemReserved
                    )
                })
                .count() as u32,
        },
        firewall: FirewallStats {
            nat_rules: nat.len() as u32,
            security_policies: pols.len() as u32,
        },
    })
}
