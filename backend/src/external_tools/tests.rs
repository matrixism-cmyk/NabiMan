use super::*;
use std::os::unix::fs::PermissionsExt;

fn fake_bin(dir: &Path, name: &str, script: &str) {
    let p = dir.join(name);
    std::fs::write(&p, format!("#!/bin/sh\n{script}\n")).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn tools_are_found_on_the_given_path_and_missing_ones_say_what_is_off() {
    let dir = std::env::temp_dir().join(format!("nabiman-tools-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    fake_bin(&dir, "tmux", "echo 'tmux 9.9-fake'");
    fake_bin(&dir, "ssh", "exit 0");
    fake_bin(&dir, "socat", "exit 0");
    std::fs::write(dir.join("sshpass"), "not runnable").unwrap();
    let path = dir.display().to_string();
    let get = move |name: &str| match name {
        "PATH" => Some(path.clone()),
        "NABIMAN_RHWP" => Some("/nonexistent/rhwp".into()),
        "NABIMAN_SOFFICE" => Some("/nonexistent/soffice.bin".into()),
        _ => None,
    };
    // A port nothing listens on: bind one, note it, let it go.
    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().to_string();
    let tools = detect(&Sources { get: &get, guacd: closed });
    let by = |n: &str| tools.iter().find(|t| t.name == n).unwrap().clone();

    let tmux = by("tmux");
    assert!(tmux.found && tmux.version.as_deref() == Some("tmux 9.9-fake") && tmux.features == ["terminal"]);
    assert!(by("ssh").found && by("socat").found);
    assert!(!by("sshpass").found, "a file that cannot be run is not the program");
    for gone in ["ssh-keygen", "ffmpeg", "guacd", "soffice", "rhwp"] {
        assert!(!by(gone).found, "{gone}");
    }
    assert_eq!(by("rhwp").path.as_deref(), Some("/nonexistent/rhwp"), "the setting is reported as set");
    assert_eq!(by("guacd").features, ["rdp_console"]);
    let line = summary(&tools);
    assert!(line.starts_with("External programs: tmux 9.9-fake, ssh, socat"), "{line}");
    assert!(line.contains("guacd (RDP consoles cannot be opened)") && line.contains("soffice (Office documents are not previewed)"), "{line}");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let open = listener.local_addr().unwrap().to_string();
    let tools = detect(&Sources { get: &get, guacd: open });
    assert!(tools.iter().any(|t| t.name == "guacd" && t.found));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn every_tool_names_what_it_is_for() {
    let get = |name: &str| (name == "PATH").then(|| "/nonexistent-nabiman".to_string());
    let tools = detect(&Sources { get: &get, guacd: "127.0.0.1:1".into() });
    let names: Vec<&str> = tools.iter().map(|t| t.name).collect();
    assert_eq!(names, ["tmux", "ssh", "sshpass", "ssh-keygen", "socat", "guacd", "soffice", "rhwp", "ffmpeg", "claude"]);
    assert!(tools.iter().all(|t| !t.missing.is_empty()));
}
