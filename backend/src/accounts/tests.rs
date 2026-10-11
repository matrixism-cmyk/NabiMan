use super::*;
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf, time::Duration};
struct Fixture {
    dir: PathBuf,
    ops: Operations,
}
impl Fixture {
    fn new() -> Self {
        let dir =
            std::env::temp_dir().join(format!("nabiman-account-fixture-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
        let home = dir.join("home");
        fs::create_dir(&home).unwrap();
        fs::create_dir(home.join("alice")).unwrap();
        fs::write(home.join("alice/canary"), "keep").unwrap();
        let passwd = dir.join("passwd");
        fs::write(
            &passwd,
            format!(
                "root:x:0:0::/root:/bin/bash\nalice:x:1001:1001::{}:/bin/bash\n",
                home.join("alice").display()
            ),
        )
        .unwrap();
        let shells = dir.join("shells");
        fs::write(&shells, "/bin/bash\n/bin/sh\n").unwrap();
        let ops = Operations {
            passwd: passwd.clone(),
            shells,
            home,
            useradd: dir.join("useradd"),
            userdel: dir.join("userdel"),
            chpasswd: dir.join("chpasswd"),
            timeout: Duration::from_secs(2),
            authorize: Box::new(|| true),
            verify_identity: Box::new(|_| true),
        };
        let common=format!("#!/usr/bin/python3\nimport sys,pathlib,json,shutil\nroot=pathlib.Path({:?})\np=root/'passwd'\na=sys.argv[1:]\n(root/(pathlib.Path(sys.argv[0]).name+'.args')).write_text(json.dumps(a))\n",dir);
        fs::write(&ops.useradd,format!("{common}name=a[-1]\nhome=a[a.index('-d')+1]\nshell=a[a.index('-s')+1]\npathlib.Path(home).mkdir()\np.write_text(p.read_text()+f'{{name}}:x:1200:1200::{{home}}:{{shell}}\\n')\n")).unwrap();
        fs::write(&ops.userdel,format!("{common}name=a[-1]\nrows=p.read_text().splitlines()\nrow=next(r for r in rows if r.split(':')[0]==name)\np.write_text('\\n'.join(r for r in rows if r.split(':')[0]!=name)+'\\n')\nif '-r' in a: shutil.rmtree(row.split(':')[5])\n")).unwrap();
        fs::write(
            &ops.chpasswd,
            format!("{common}(root/'password.input').write_text(sys.stdin.read())\n"),
        )
        .unwrap();
        for p in [&ops.useradd, &ops.userdel, &ops.chpasswd] {
            fs::set_permissions(p, fs::Permissions::from_mode(0o700)).unwrap();
        }
        Self { dir, ops }
    }
    fn change(&self, name: &str, pw: &str, uid: u32) -> Result<String, String> {
        self.ops.password(ChangePasswordRequest {
            username: name.into(),
            new_password: pw.into(),
            expected_uid: uid,
        })
    }
    fn delete(&self, remove: bool, confirm: Option<String>) -> Result<String, String> {
        self.ops.delete(DeleteAccountRequest {
            username: "alice".into(),
            expected_uid: 1001,
            remove_home: remove,
            confirm_home: confirm,
        })
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}
#[test]
fn rejects_line_and_option_injection_before_any_tool() {
    let f = Fixture::new();
    for name in [
        "",
        "-r",
        "--root",
        "root\nother",
        "root:other",
        "a/b",
        "a b",
        "Ålice",
    ] {
        assert!(f.change(name, "fixture-password-long", 1001).is_err());
    }
    for password in [
        "short",
        "fixture-password\nroot:injected",
        "fixture-password\rnext",
        "fixture-password\0end",
        "fixture-password\tend",
    ] {
        assert!(f.change("alice", password, 1001).is_err());
    }
    assert!(!f.dir.join("chpasswd.args").exists());
    assert!(!f.dir.join("password.input").exists());
}
#[test]
fn exact_uid_and_protected_aliases_are_enforced() {
    let f = Fixture::new();
    assert!(f.change("alice", "fixture-password-long", 1002).is_err());
    assert!(f.change("root", "fixture-password-long", 0).is_err());
    let mut content = fs::read_to_string(&f.ops.passwd).unwrap();
    content.push_str("alias:x:1001:1001::/home/alias:/bin/bash\n");
    fs::write(&f.ops.passwd, content).unwrap();
    assert!(f.change("alice", "fixture-password-long", 1001).is_err());
    assert!(!f.dir.join("chpasswd.args").exists());
}
#[test]
fn password_is_single_stdin_record_and_never_an_argument() {
    let f = Fixture::new();
    f.change("alice", "fixture:password-long", 1001).unwrap();
    assert_eq!(
        fs::read_to_string(f.dir.join("password.input")).unwrap(),
        "alice:fixture:password-long\n"
    );
    assert_eq!(
        fs::read_to_string(f.dir.join("chpasswd.args")).unwrap(),
        "[]"
    );
}
#[test]
fn delete_keeps_home_by_default_and_passes_an_option_boundary() {
    let f = Fixture::new();
    f.delete(false, None).unwrap();
    assert_eq!(
        fs::read_to_string(f.ops.home.join("alice/canary")).unwrap(),
        "keep"
    );
    assert_eq!(
        serde_json::from_str::<Vec<String>>(
            &fs::read_to_string(f.dir.join("userdel.args")).unwrap()
        )
        .unwrap(),
        ["--", "alice"]
    );
    assert!(f
        .ops
        .inventory()
        .unwrap()
        .iter()
        .all(|u| u.username != "alice"));
}
#[test]
fn home_removal_requires_exact_confirmation_and_dedicated_non_link_directory() {
    let f = Fixture::new();
    assert!(f.delete(true, None).is_err());
    assert!(f.delete(true, Some("/wrong".into())).is_err());
    assert!(!f.dir.join("userdel.args").exists());
    let home = f.ops.home.join("alice");
    fs::remove_dir_all(&home).unwrap();
    std::os::unix::fs::symlink(&f.dir, &home).unwrap();
    assert!(f.delete(true, Some(home.display().to_string())).is_err());
    fs::remove_file(&home).unwrap();
    fs::create_dir(&home).unwrap();
    f.delete(true, Some(home.display().to_string())).unwrap();
    assert!(!home.exists());
}
#[test]
fn creates_only_approved_shell_and_fresh_home_then_reports_partial_password_failure() {
    let f = Fixture::new();
    let create = |shell: &str| CreateAccountRequest {
        username: "bob".into(),
        password: "fixture-password-long".into(),
        shell: Some(shell.into()),
    };
    assert!(f.ops.create(create("/tmp/arbitrary-shell")).is_err());
    assert!(!f.dir.join("useradd.args").exists());
    fs::create_dir(f.ops.home.join("bob")).unwrap();
    assert!(f.ops.create(create("/bin/bash")).is_err());
    fs::remove_dir(f.ops.home.join("bob")).unwrap();
    fs::write(&f.ops.chpasswd, "#!/bin/sh\nexit 1\n").unwrap();
    let message = f.ops.create(create("/bin/bash")).unwrap_err();
    assert!(message.contains("password setup was not confirmed"));
    assert!(f
        .ops
        .inventory()
        .unwrap()
        .iter()
        .any(|u| u.username == "bob"));
    assert!(f.ops.home.join("bob").exists());
    assert!(!f.dir.join("userdel.args").exists());
}
#[test]
fn timeout_and_tool_errors_do_not_expose_output_or_claim_rollback() {
    let f = Fixture::new();
    fs::write(
        &f.ops.chpasswd,
        "#!/bin/sh\necho PRIVATE_FIXTURE_DIAGNOSTIC >&2\nsleep 2\n",
    )
    .unwrap();
    let error = operations::run(
        &f.ops.chpasswd,
        &[],
        Some("fixture input\n"),
        Duration::from_millis(30),
    )
    .unwrap_err();
    assert!(error.contains("outcome unknown"));
    assert!(!error.contains("PRIVATE_FIXTURE"));
    let error =
        operations::run(&f.dir.join("missing"), &[], None, Duration::from_secs(1)).unwrap_err();
    assert!(error.contains("no command dispatched"));
}
#[test]
fn inventory_rejects_incomplete_duplicate_and_unreadable_state() {
    for input in [
        "",
        "broken",
        "a:x:bad:1000::/home/a:/bin/sh",
        "a:x:1000:1000::/home/a:/bin/sh\na:x:1001:1001::/home/b:/bin/sh",
    ] {
        assert!(inventory::parse(input).is_err());
    }
    let f = Fixture::new();
    fs::remove_file(&f.ops.passwd).unwrap();
    assert!(f.ops.inventory().is_err());
    std::os::unix::fs::symlink(&f.ops.shells, &f.ops.passwd).unwrap();
    assert!(f.ops.inventory().is_err());
}

#[test]
fn revoked_authorization_stops_before_each_command_without_compensating_deletion() {
    let mut f = Fixture::new();
    f.ops.authorize = Box::new(|| false);
    assert!(f.change("alice", "fixture-password-long", 1001).is_err());
    assert!(!f.dir.join("chpasswd.args").exists());
    let count = std::sync::atomic::AtomicUsize::new(0);
    f.ops.authorize =
        Box::new(move || count.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0);
    assert!(f
        .ops
        .create(CreateAccountRequest {
            username: "bob".into(),
            password: "fixture-password-long".into(),
            shell: Some("/bin/bash".into())
        })
        .is_err());
    assert!(f.ops.home.join("bob").exists());
    assert!(!f.dir.join("chpasswd.args").exists());
    assert!(!f.dir.join("userdel.args").exists());
}

#[test]
fn nss_disagreement_prevents_name_based_tools_from_touching_another_identity() {
    let mut f = Fixture::new();
    f.ops.verify_identity = Box::new(|_| false);
    assert!(f.change("alice", "fixture-password-long", 1001).is_err());
    assert!(f.delete(false, None).is_err());
    assert!(!f.dir.join("chpasswd.args").exists());
    assert!(!f.dir.join("userdel.args").exists());
}
