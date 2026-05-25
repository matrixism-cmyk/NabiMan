use serde_json::{json, Value};

pub struct NetworkPolicyTemplate {
    pub name: String,
    pub description: String,
    pub spec: Value,
}

/// 5 core NetworkPolicies applied to every tenant namespace,
/// matching operator-kit/scripts/01-init-tenant.sh templates.
pub fn default_tenant_policies(namespace: &str) -> Vec<NetworkPolicyTemplate> {
    vec![
        deny_all_ingress(namespace),
        allow_same_namespace(namespace),
        allow_ingress_from_ingress_controller(namespace),
        allow_dns_egress(namespace),
        allow_cluster_egress(namespace),
    ]
}

fn policy(name: &str, namespace: &str, spec: Value) -> NetworkPolicyTemplate {
    NetworkPolicyTemplate {
        name: name.to_string(),
        description: default_description(name),
        spec: json!({
            "apiVersion": "networking.k8s.io/v1",
            "kind": "NetworkPolicy",
            "metadata": {
                "name": name,
                "namespace": namespace,
                "labels": {
                    "managed-by": "nabiman",
                    "mec.policy": "tenant-default"
                }
            },
            "spec": spec
        }),
    }
}

fn default_description(name: &str) -> String {
    match name {
        "deny-all-ingress" => "기본 deny — 명시 정책 없는 트래픽 차단",
        "allow-same-namespace" => "같은 네임스페이스 내 파드 간 통신 허용",
        "allow-ingress-from-nginx" => "ingress-nginx 컨트롤러에서의 인입 허용",
        "allow-dns-egress" => "kube-dns / coredns 로의 DNS 조회 허용",
        "allow-cluster-egress" => "클러스터 내부/외부로 이그레스 허용",
        _ => "nabiman tenant default",
    }
    .to_string()
}

fn deny_all_ingress(namespace: &str) -> NetworkPolicyTemplate {
    policy(
        "deny-all-ingress",
        namespace,
        json!({
            "podSelector": {},
            "policyTypes": ["Ingress"],
            "ingress": []
        }),
    )
}

fn allow_same_namespace(namespace: &str) -> NetworkPolicyTemplate {
    policy(
        "allow-same-namespace",
        namespace,
        json!({
            "podSelector": {},
            "policyTypes": ["Ingress"],
            "ingress": [{
                "from": [{
                    "podSelector": {}
                }]
            }]
        }),
    )
}

fn allow_ingress_from_ingress_controller(namespace: &str) -> NetworkPolicyTemplate {
    policy(
        "allow-ingress-from-nginx",
        namespace,
        json!({
            "podSelector": {},
            "policyTypes": ["Ingress"],
            "ingress": [{
                "from": [{
                    "namespaceSelector": {
                        "matchLabels": {
                            "kubernetes.io/metadata.name": "ingress-nginx"
                        }
                    }
                }]
            }]
        }),
    )
}

fn allow_dns_egress(namespace: &str) -> NetworkPolicyTemplate {
    policy(
        "allow-dns-egress",
        namespace,
        json!({
            "podSelector": {},
            "policyTypes": ["Egress"],
            "egress": [{
                "to": [{
                    "namespaceSelector": {
                        "matchLabels": {
                            "kubernetes.io/metadata.name": "kube-system"
                        }
                    },
                    "podSelector": {
                        "matchLabels": {
                            "k8s-app": "kube-dns"
                        }
                    }
                }],
                "ports": [
                    { "protocol": "UDP", "port": 53 },
                    { "protocol": "TCP", "port": 53 }
                ]
            }]
        }),
    )
}

fn allow_cluster_egress(namespace: &str) -> NetworkPolicyTemplate {
    policy(
        "allow-cluster-egress",
        namespace,
        json!({
            "podSelector": {},
            "policyTypes": ["Egress"],
            "egress": [{
                "to": [],
                "ports": []
            }]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policies_count() {
        let ps = default_tenant_policies("ygram-poc");
        assert_eq!(ps.len(), 5);
    }

    #[test]
    fn policy_contains_namespace() {
        let ps = default_tenant_policies("witches-poc");
        let first = &ps[0];
        let md = first.spec["metadata"].clone();
        assert_eq!(md["namespace"], "witches-poc");
    }

    #[test]
    fn deny_has_empty_ingress() {
        let ps = default_tenant_policies("x");
        let deny = ps.iter().find(|p| p.name == "deny-all-ingress").unwrap();
        assert!(deny.spec["spec"]["ingress"].as_array().unwrap().is_empty());
    }
}
