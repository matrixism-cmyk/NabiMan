use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuOverview {
    pub total_slots: u32,
    pub allocated_slots: u32,
    pub available_slots: u32,
    pub by_model: Vec<GpuByModel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuByModel {
    pub model: String,
    pub total_slots: u32,
    pub allocated_slots: u32,
    pub available_slots: u32,
    pub nodes: Vec<String>,
    pub sharing_strategy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuAllocation {
    pub node: String,
    pub tenant: Option<String>,
    pub allocated: u32,
    pub total: u32,
    pub model: String,
    pub pods: Vec<GpuPodUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuPodUsage {
    pub namespace: String,
    pub pod: String,
    pub gpu_requests: u32,
    pub gpu_utilization_percent: Option<f32>,
    pub gpu_memory_used_mb: Option<u32>,
}

impl GpuOverview {
    pub fn from_models(by_model: Vec<GpuByModel>) -> Self {
        let total_slots: u32 = by_model.iter().map(|m| m.total_slots).sum();
        let allocated_slots: u32 = by_model.iter().map(|m| m.allocated_slots).sum();
        Self {
            total_slots,
            allocated_slots,
            available_slots: total_slots.saturating_sub(allocated_slots),
            by_model,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overview_aggregates() {
        let models = vec![
            GpuByModel {
                model: "T4".into(),
                total_slots: 12,
                allocated_slots: 8,
                available_slots: 4,
                nodes: vec!["mec-cp02".into()],
                sharing_strategy: "time-slicing".into(),
            },
            GpuByModel {
                model: "A40".into(),
                total_slots: 16,
                allocated_slots: 16,
                available_slots: 0,
                nodes: vec!["mec-wn02".into()],
                sharing_strategy: "time-slicing".into(),
            },
        ];
        let o = GpuOverview::from_models(models);
        assert_eq!(o.total_slots, 28);
        assert_eq!(o.allocated_slots, 24);
        assert_eq!(o.available_slots, 4);
    }
}
