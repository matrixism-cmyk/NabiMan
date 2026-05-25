use super::util::{from_service_error, list_response, ok_response};
use crate::mec::MecState;
use crate::models::mec::{GpuAllocation, GpuByModel, GpuOverview};
use actix_web::{web, HttpResponse};
use std::collections::HashMap;

pub async fn overview(state: web::Data<MecState>) -> HttpResponse {
    let nodes = match state.services.kube.list_nodes().await {
        Ok(n) => n,
        Err(e) => return from_service_error(e),
    };
    let mut by_model: HashMap<String, GpuByModel> = HashMap::new();
    for n in &nodes {
        if let Some(g) = &n.gpu_info {
            let entry = by_model.entry(g.model.clone()).or_insert(GpuByModel {
                model: g.model.clone(),
                total_slots: 0,
                allocated_slots: 0,
                available_slots: 0,
                nodes: Vec::new(),
                sharing_strategy: g.sharing_strategy.clone().unwrap_or_else(|| "none".into()),
            });
            entry.total_slots += g.total_slots;
            entry.nodes.push(n.name.clone());
            if n.current_tenant.is_some() {
                entry.allocated_slots += g.total_slots;
            }
        }
    }
    for entry in by_model.values_mut() {
        entry.available_slots = entry.total_slots.saturating_sub(entry.allocated_slots);
    }
    let overview = GpuOverview::from_models(by_model.into_values().collect());
    ok_response(overview)
}

pub async fn allocations(state: web::Data<MecState>) -> HttpResponse {
    let nodes = match state.services.kube.list_nodes().await {
        Ok(n) => n,
        Err(e) => return from_service_error(e),
    };
    let allocations: Vec<GpuAllocation> = nodes
        .into_iter()
        .filter_map(|n| {
            n.gpu_info.as_ref().map(|g| GpuAllocation {
                node: n.name.clone(),
                tenant: n.current_tenant.clone(),
                allocated: if n.current_tenant.is_some() {
                    g.total_slots
                } else {
                    0
                },
                total: g.total_slots,
                model: g.model.clone(),
                pods: vec![],
            })
        })
        .collect();
    list_response(allocations)
}

pub async fn usage(state: web::Data<MecState>) -> HttpResponse {
    match state.services.kube.list_nodes().await {
        Ok(nodes) => {
            let out: Vec<_> = nodes
                .into_iter()
                .filter_map(|n| {
                    n.gpu_info.as_ref().map(|g| {
                        serde_json::json!({
                            "node": n.name,
                            "model": g.model,
                            "total_slots": g.total_slots,
                            "utilization_percent": null,
                        })
                    })
                })
                .collect();
            list_response(out)
        }
        Err(e) => from_service_error(e),
    }
}
