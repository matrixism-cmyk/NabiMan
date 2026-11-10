use super::*;

struct Dir(std::path::PathBuf);
impl Dir {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("nabiman-bootstrap-{}", uuid::Uuid::new_v4())))
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}
const PASSWORD: &str = "provisioned-admin-pass-1";

#[test]
fn the_options_come_in_either_order_and_nothing_else_is_taken() {
    let want = Request { username: "owner".into(), password_file: "/run/pw".into() };
    assert_eq!(parse(&args(&["--bootstrap-admin", "owner", "--password-file", "/run/pw"])), Ok(want));
    assert!(parse(&args(&["--password-file", "-", "--bootstrap-admin", "owner"])).is_ok());
    for bad in [
        &["--bootstrap-admin", "owner"][..],
        &["--bootstrap-admin", "owner", "--password-file"],
        &["--bootstrap-admin", "a", "--bootstrap-admin", "b", "--password-file", "x"],
        &["--bootstrap-admin", "owner", "--password-file", "x", "--port", "1"],
        &["--bootstrap-admin", "owner", "--password", "secret"],
    ] {
        assert!(matches!(parse(&args(bad)), Err(Failure::Invalid(_))), "{bad:?}");
    }
}

#[test]
fn an_empty_directory_gets_one_super_administrator_who_can_sign_in() {
    let dir = Dir::new();
    create(&dir.0, "owner", PASSWORD).unwrap();
    let file = dir.0.join("users.json");
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
    let users: Vec<crate::models::NabimanUser> = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].username, "owner");
    assert!(users[0].super_admin() && !users[0].scope_restricted);
    assert!(bcrypt::verify(PASSWORD, &users[0].password_hash).unwrap());
    assert!(!std::fs::read_to_string(&file).unwrap().contains(PASSWORD));
}

#[test]
fn a_directory_with_users_is_left_exactly_as_it_was() {
    let dir = Dir::new();
    create(&dir.0, "owner", PASSWORD).unwrap();
    let before = std::fs::read(dir.0.join("users.json")).unwrap();
    assert_eq!(create(&dir.0, "intruder", "another-long-password"), Err(Failure::HasUsers));
    assert_eq!(std::fs::read(dir.0.join("users.json")).unwrap(), before);
    assert_eq!(Failure::HasUsers.code(), 2);
}

#[test]
fn a_directory_a_server_holds_is_not_touched() {
    let dir = Dir::new();
    let _server = crate::data_dir::lock::acquire(&dir.0).unwrap();
    let refused = create(&dir.0, "owner", PASSWORD).unwrap_err();
    assert_eq!(refused.code(), 3);
    assert!(!dir.0.join("users.json").exists());
}

#[test]
fn names_and_passwords_the_web_form_refuses_are_refused() {
    let dir = Dir::new();
    let long = "a".repeat(73);
    for (name, password) in [
        ("o", PASSWORD),
        ("owner name", PASSWORD),
        ("../owner", PASSWORD),
        ("owner", "short"),
        ("owner", "changeme"),
        ("owner", long.as_str()),
    ] {
        assert!(matches!(create(&dir.0, name, password), Err(Failure::Invalid(_))), "{name}");
    }
    assert!(!dir.0.join("users.json").exists());
}

#[test]
fn the_password_file_loses_one_trailing_newline_only() {
    let dir = Dir::new();
    std::fs::create_dir_all(&dir.0).unwrap();
    let file = dir.0.join("pw");
    for (text, want) in [("pass word  \n", "pass word  "), ("crlf\r\n", "crlf"), ("two\n\n", "two\n"), ("none", "none")] {
        std::fs::write(&file, text).unwrap();
        assert_eq!(read_password(file.to_str().unwrap()).unwrap(), want);
    }
    std::fs::write(&file, "x".repeat(5000)).unwrap();
    assert!(read_password(file.to_str().unwrap()).is_err());
    assert!(read_password(dir.0.join("missing").to_str().unwrap()).is_err());
}
