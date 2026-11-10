use super::tmux_conf_body;

#[test]
fn mouse_apps_get_ctrl_drag_and_right_click_menus_stay_open() {
    let body = tmux_conf_body(5000);
    assert!(body.contains("bind -T root C-MouseDrag1Pane if -F '#{mouse_any_flag}' { send -M } { new-pane -M }"));
    let menus: Vec<&str> = body.lines().filter(|l| l.contains("MouseDown3")).filter(|l| l.contains("display-menu")).collect();
    assert_eq!(menus.len(), 6, "pane, status and status-left menus, with and without Alt");
    assert!(menus.iter().all(|l| !l.contains("display-menu -T") && !l.contains("display-menu -t")), "every menu has -O");
    // tmux 로 직접 읽어 보는 확인용 (cargo test 뒤 `tmux -f` 로 읽어 본다)
    std::fs::write(std::env::temp_dir().join("nabiman-tmux-conf-test.conf"), body).unwrap();
}
