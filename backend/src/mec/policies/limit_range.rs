use crate::models::mec::ResourceQuota;
use serde_json::{json, Value};

pub struct LimitRangeTemplate {
    pub name: String,
    pub spec: Value,
}

/// LimitRange with sensible defaults derived from the tenant's quota.
/// - Container-level defaults: 500m CPU / 512Mi memory
/// - Max per container: clamped to 1/4 of tenant CPU/memory limit
pub fn default_limit_range(namespace: &str, quota: &ResourceQuota) -> LimitRangeTemplate {
    let container_cpu_max = clamp_cpu_quarter(&quota.cpu_limits);
    let container_mem_max = clamp_memory_quarter(&quota.memory_limits);
    let spec = json!({
        "apiVersion": "v1",
        "kind": "LimitRange",
        "metadata": {
            "name": "tenant-limits",
            "namespace": namespace,
            "labels": {
                "managed-by": "nabiman"
            }
        },
        "spec": {
            "limits": [
                {
                    "type": "Container",
                    "default": {
                        "cpu": "500m",
                        "memory": "512Mi"
                    },
                    "defaultRequest": {
                        "cpu": "100m",
                        "memory": "128Mi"
                    },
                    "max": {
                        "cpu": container_cpu_max,
                        "memory": container_mem_max
                    },
                    "min": {
                        "cpu": "10m",
                        "memory": "16Mi"
                    }
                },
                {
                    "type": "PersistentVolumeClaim",
                    "min": { "storage": "1Gi" },
                    "max": { "storage": quota.storage.clone() }
                }
            ]
        }
    });
    LimitRangeTemplate {
        name: "tenant-limits".into(),
        spec,
    }
}

/// Returns quarter of the cpu quantity, preserving unit format.
/// "64" -> "16", "500m" -> "125m"; falls back to input when unparsable.
fn clamp_cpu_quarter(input: &str) -> String {
    if let Some(num) = input.strip_suffix('m') {
        match num.parse::<u64>() {
            Ok(v) => return format!("{}m", v / 4),
            Err(_) => return input.to_string(),
        }
    }
    match input.parse::<f64>() {
        Ok(v) => {
            let q = v / 4.0;
            if q >= 1.0 {
                format!("{}", q.floor() as u64)
            } else {
                format!("{}m", (q * 1000.0) as u64)
            }
        }
        Err(_) => input.to_string(),
    }
}

/// Quarter of a memory quantity. Supports Mi / Gi / Ti.
fn clamp_memory_quarter(input: &str) -> String {
    for unit in ["Ti", "Gi", "Mi", "Ki", "T", "G", "M", "K"] {
        if let Some(num) = input.strip_suffix(unit) {
            if let Ok(v) = num.parse::<u64>() {
                return format!("{}{}", v.max(4) / 4, unit);
            }
        }
    }
    match input.parse::<u64>() {
        Ok(v) => format!("{}", v.max(4) / 4),
        Err(_) => input.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quarter_cpu_whole() {
        assert_eq!(clamp_cpu_quarter("64"), "16");
    }

    #[test]
    fn quarter_cpu_milli() {
        assert_eq!(clamp_cpu_quarter("500m"), "125m");
    }

    #[test]
    fn quarter_memory() {
        assert_eq!(clamp_memory_quarter("128Gi"), "32Gi");
        assert_eq!(clamp_memory_quarter("500Mi"), "125Mi");
    }

    #[test]
    fn limit_range_contains_pvc_max() {
        let q = ResourceQuota::standard();
        let lr = default_limit_range("ns", &q);
        let limits = lr.spec["spec"]["limits"].as_array().unwrap();
        let pvc = &limits[1];
        assert_eq!(pvc["type"], "PersistentVolumeClaim");
        assert_eq!(pvc["max"]["storage"], "500Gi");
    }
}
