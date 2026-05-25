use super::client::upstream_err;
use super::nodes::map_not_found;
use crate::mec::services::ServiceResult;
use k8s_openapi::api::core::v1::LimitRange;
use k8s_openapi::api::networking::v1::NetworkPolicy;
use k8s_openapi::api::rbac::v1::{PolicyRule, Role, RoleBinding, RoleRef, Subject};
use kube::{
    api::{Api, Patch, PatchParams, PostParams},
    Client,
};

pub async fn apply_network_policy(
    client: &Client,
    namespace: &str,
    name: &str,
    spec_json: &str,
) -> ServiceResult<()> {
    let api: Api<NetworkPolicy> = Api::namespaced(client.clone(), namespace);

    // Parse provided spec, or fall back to a minimal "allow all egress" policy.
    let policy: NetworkPolicy = if spec_json.trim().is_empty() || spec_json == "{}" {
        serde_json::from_value(default_egress_policy(name)).map_err(|e| {
            crate::mec::services::ServiceError::Internal(format!(
                "build network policy: {}",
                e
            ))
        })?
    } else {
        serde_json::from_str(spec_json).map_err(|e| {
            crate::mec::services::ServiceError::InvalidInput(format!(
                "network policy json: {}",
                e
            ))
        })?
    };

    match api.create(&PostParams::default(), &policy).await {
        Ok(_) => Ok(()),
        Err(kube::Error::Api(ae)) if ae.code == 409 => {
            let patch = serde_json::to_value(&policy).unwrap_or_default();
            api.patch(name, &PatchParams::default(), &Patch::Merge(&patch))
                .await
                .map_err(upstream_err)?;
            Ok(())
        }
        Err(e) => Err(map_not_found(e, &format!("network policy {}", name))),
    }
}

fn default_egress_policy(name: &str) -> serde_json::Value {
    serde_json::json!({
        "apiVersion": "networking.k8s.io/v1",
        "kind": "NetworkPolicy",
        "metadata": { "name": name },
        "spec": {
            "podSelector": {},
            "policyTypes": ["Egress"],
            "egress": [{
                "to": [],
                "ports": []
            }]
        }
    })
}

pub async fn apply_limit_range(
    client: &Client,
    namespace: &str,
    spec_json: &str,
) -> ServiceResult<()> {
    let api: Api<LimitRange> = Api::namespaced(client.clone(), namespace);
    let lr: LimitRange = serde_json::from_str(spec_json).map_err(|e| {
        crate::mec::services::ServiceError::InvalidInput(format!("limit range json: {}", e))
    })?;
    let name = lr
        .metadata
        .name
        .clone()
        .unwrap_or_else(|| "tenant-limits".into());
    match api.create(&PostParams::default(), &lr).await {
        Ok(_) => Ok(()),
        Err(kube::Error::Api(ae)) if ae.code == 409 => {
            let patch = serde_json::to_value(&lr).unwrap_or_default();
            api.patch(&name, &PatchParams::default(), &Patch::Merge(&patch))
                .await
                .map_err(upstream_err)?;
            Ok(())
        }
        Err(e) => Err(map_not_found(e, &format!("limit range {}", name))),
    }
}

pub async fn apply_rbac(client: &Client, namespace: &str, user: &str) -> ServiceResult<()> {
    let role_name = "tenant-operator";
    let binding_name = format!("{}-binding", user);

    let roles: Api<Role> = Api::namespaced(client.clone(), namespace);
    let bindings: Api<RoleBinding> = Api::namespaced(client.clone(), namespace);

    let role = Role {
        metadata: kube::core::ObjectMeta {
            name: Some(role_name.into()),
            namespace: Some(namespace.into()),
            ..Default::default()
        },
        rules: Some(vec![
            PolicyRule {
                api_groups: Some(vec!["".into(), "apps".into(), "batch".into()]),
                resources: Some(vec!["*".into()]),
                verbs: vec!["get", "list", "watch", "create", "update", "patch", "delete"]
                    .into_iter()
                    .map(String::from)
                    .collect(),
                ..Default::default()
            },
        ]),
    };

    let binding = RoleBinding {
        metadata: kube::core::ObjectMeta {
            name: Some(binding_name.clone()),
            namespace: Some(namespace.into()),
            ..Default::default()
        },
        role_ref: RoleRef {
            api_group: "rbac.authorization.k8s.io".into(),
            kind: "Role".into(),
            name: role_name.into(),
        },
        subjects: Some(vec![Subject {
            api_group: Some("rbac.authorization.k8s.io".into()),
            kind: "User".into(),
            name: user.into(),
            namespace: None,
        }]),
    };

    upsert_role(&roles, role_name, role).await?;
    upsert_binding(&bindings, &binding_name, binding).await?;
    Ok(())
}

async fn upsert_role(api: &Api<Role>, name: &str, role: Role) -> ServiceResult<()> {
    match api.create(&PostParams::default(), &role).await {
        Ok(_) => Ok(()),
        Err(kube::Error::Api(ae)) if ae.code == 409 => {
            let patch = serde_json::to_value(&role).unwrap_or_default();
            api.patch(name, &PatchParams::default(), &Patch::Merge(&patch))
                .await
                .map_err(upstream_err)?;
            Ok(())
        }
        Err(e) => Err(upstream_err(e)),
    }
}

async fn upsert_binding(
    api: &Api<RoleBinding>,
    name: &str,
    binding: RoleBinding,
) -> ServiceResult<()> {
    match api.create(&PostParams::default(), &binding).await {
        Ok(_) => Ok(()),
        Err(kube::Error::Api(ae)) if ae.code == 409 => {
            let patch = serde_json::to_value(&binding).unwrap_or_default();
            api.patch(name, &PatchParams::default(), &Patch::Merge(&patch))
                .await
                .map_err(upstream_err)?;
            Ok(())
        }
        Err(e) => Err(upstream_err(e)),
    }
}
