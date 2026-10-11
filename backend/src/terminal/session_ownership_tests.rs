use super::*;
#[test]
fn immutable_owner_survives_display_changes_but_not_same_name_replacement_or_legacy() {
    let mut session:TmuxSession=serde_json::from_value(serde_json::json!({"tmux_name":"nb_fixture","owner":"same-name","owner_id":"old-id","shared":true,"label":"local"})).unwrap();
    let owner:crate::models::NabimanUser=serde_json::from_value(serde_json::json!({"id":"old-id","username":"same-name","password_hash":"fixture","role":"admin","created_at":"now"})).unwrap();
    let users = Arc::new(Mutex::new(vec![owner]));
    assert!(visible_to(&session, "other-root-id", &users));
    session.owner = "historical-display".into();
    users.lock().unwrap()[0].username = "renamed-account".into();
    assert!(visible_to(&session, "old-id", &users));
    users.lock().unwrap()[0].id = "new-id".into();
    users.lock().unwrap()[0].username = "same-name".into();
    assert!(!session.owned_by("new-id"));
    assert!(!visible_to(&session, "new-id", &users));
    assert!(!visible_to(&session, "other-root-id", &users));
    session.owner_id.clear();
    assert!(!session.owned_by("old-id"));
    assert!(!visible_to(&session, "new-id", &users));
    let restored: TmuxSession =
        serde_json::from_slice(&serde_json::to_vec(&session).unwrap()).unwrap();
    assert!(restored.owner_id.is_empty());
}
