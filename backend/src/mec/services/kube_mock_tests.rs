use super::*;

#[tokio::test]
async fn mock_list_nodes() {
    let m = KubeMock::new();
    let nodes = m.list_nodes().await.unwrap();
    assert!(nodes.len() >= 4);
}

#[tokio::test]
async fn mock_create_delete_namespace() {
    let m = KubeMock::new();
    m.create_namespace("foo", &[]).await.unwrap();
    assert!(m
        .list_namespaces()
        .await
        .unwrap()
        .iter()
        .any(|n| n.name == "foo"));
    m.delete_namespace("foo").await.unwrap();
    assert!(!m
        .list_namespaces()
        .await
        .unwrap()
        .iter()
        .any(|n| n.name == "foo"));
}

#[tokio::test]
async fn mock_taint_add_remove() {
    let m = KubeMock::new();
    m.patch_taints(
        "mec-cp01",
        &[Taint {
            key: "dedicated".into(),
            value: Some("test".into()),
            effect: TaintEffect::NoSchedule,
        }],
        &[],
    )
    .await
    .unwrap();
    let n = m.get_node("mec-cp01").await.unwrap();
    assert!(n.taints.iter().any(|t| t.key == "dedicated"));
}
