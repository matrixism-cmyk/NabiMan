use serde_json::{json, Value};

/// Returns (Deployment, Service) JSON for a single ubuntu-ssh pod.
pub fn ubuntu_ssh_manifests(
    namespace: &str,
    tenant_id: &str,
    ssh_user: &str,
    ssh_password_hash: &str,
) -> (Value, Value) {
    let deploy = json!({
        "apiVersion": "apps/v1",
        "kind": "Deployment",
        "metadata": {
            "name": "ubuntu-ssh",
            "namespace": namespace,
            "labels": {
                "app": "ubuntu-ssh",
                "tenant": tenant_id,
                "managed-by": "nabiman"
            }
        },
        "spec": {
            "replicas": 1,
            "selector": { "matchLabels": { "app": "ubuntu-ssh" } },
            "template": {
                "metadata": { "labels": { "app": "ubuntu-ssh", "tenant": tenant_id } },
                "spec": {
                    "containers": [{
                        "name": "ubuntu-ssh",
                        "image": "linuxserver/openssh-server:latest",
                        "env": [
                            { "name": "PUID", "value": "1000" },
                            { "name": "PGID", "value": "1000" },
                            { "name": "USER_NAME", "value": ssh_user },
                            { "name": "PASSWORD_ACCESS", "value": "true" },
                            { "name": "USER_PASSWORD_HASH", "value": ssh_password_hash },
                        ],
                        "ports": [{ "containerPort": 2222, "name": "ssh" }],
                        "resources": {
                            "requests": { "cpu": "100m", "memory": "128Mi" },
                            "limits": { "cpu": "500m", "memory": "512Mi" }
                        }
                    }]
                }
            }
        }
    });
    let svc = json!({
        "apiVersion": "v1",
        "kind": "Service",
        "metadata": {
            "name": "ubuntu-ssh-svc",
            "namespace": namespace,
            "labels": { "tenant": tenant_id, "managed-by": "nabiman" }
        },
        "spec": {
            "type": "LoadBalancer",
            "selector": { "app": "ubuntu-ssh" },
            "ports": [{
                "name": "ssh",
                "port": 22,
                "targetPort": 2222,
                "protocol": "TCP"
            }]
        }
    });
    (deploy, svc)
}

/// Returns (Deployment, Service) JSON for code-server (VS Code in browser).
pub fn vscode_manifests(
    namespace: &str,
    tenant_id: &str,
    password: &str,
) -> (Value, Value) {
    let deploy = json!({
        "apiVersion": "apps/v1",
        "kind": "Deployment",
        "metadata": {
            "name": "vscode",
            "namespace": namespace,
            "labels": {
                "app": "vscode",
                "tenant": tenant_id,
                "managed-by": "nabiman"
            }
        },
        "spec": {
            "replicas": 1,
            "selector": { "matchLabels": { "app": "vscode" } },
            "template": {
                "metadata": { "labels": { "app": "vscode", "tenant": tenant_id } },
                "spec": {
                    "containers": [{
                        "name": "vscode",
                        "image": "codercom/code-server:latest",
                        "env": [
                            { "name": "PASSWORD", "value": password },
                        ],
                        "ports": [{ "containerPort": 8080, "name": "http" }],
                        "resources": {
                            "requests": { "cpu": "200m", "memory": "512Mi" },
                            "limits": { "cpu": "1", "memory": "2Gi" }
                        }
                    }]
                }
            }
        }
    });
    let svc = json!({
        "apiVersion": "v1",
        "kind": "Service",
        "metadata": {
            "name": "vscode-svc",
            "namespace": namespace,
            "labels": { "tenant": tenant_id, "managed-by": "nabiman" }
        },
        "spec": {
            "type": "LoadBalancer",
            "selector": { "app": "vscode" },
            "ports": [{
                "name": "http",
                "port": 8080,
                "targetPort": 8080,
                "protocol": "TCP"
            }]
        }
    });
    (deploy, svc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ubuntu_ssh_has_ssh_port() {
        let (d, s) = ubuntu_ssh_manifests("ns", "t", "user", "hash");
        let ports = d["spec"]["template"]["spec"]["containers"][0]["ports"].as_array().unwrap();
        assert_eq!(ports[0]["containerPort"], 2222);
        let svc_port = &s["spec"]["ports"][0];
        assert_eq!(svc_port["port"], 22);
    }

    #[test]
    fn vscode_has_http_port() {
        let (d, s) = vscode_manifests("ns", "t", "pw");
        let ports = d["spec"]["template"]["spec"]["containers"][0]["ports"].as_array().unwrap();
        assert_eq!(ports[0]["containerPort"], 8080);
        let svc_port = &s["spec"]["ports"][0];
        assert_eq!(svc_port["port"], 8080);
    }
}
