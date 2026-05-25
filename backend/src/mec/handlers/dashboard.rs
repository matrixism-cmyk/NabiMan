use super::util::{from_service_error, ok_response};
use crate::mec::MecState;
use crate::models::mec::{ClusterUsage, LiveSnapshot};
use actix_web::{web, HttpResponse};
use serde::Serialize;

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
    match build_live(&state).await {
        Ok(s) => ok_response(s),
        Err(e) => from_service_error(e),
    }
}

async fn build_live(state: &MecState) -> crate::mec::services::ServiceResult<LiveSnapshot> {
    let nodes = state.services.kube.node_metrics().await?;
    let pods = state.services.kube.pod_phase_summary().await?;
    let mut tenants = state.services.kube.tenant_usage().await?;
    tenants.truncate(12);
    let events = state.services.kube.list_events(25).await?;

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

    Ok(LiveSnapshot { cluster, nodes, pods, tenants, events })
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
