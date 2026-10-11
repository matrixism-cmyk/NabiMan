use super::{document, operations::Operations};
use crate::bounded_command as runner;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    time::{Duration, Instant},
};
struct Fixture {
    dir: PathBuf,
    ops: Operations,
}
impl Fixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("nb-cron-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(
            dir.join("passwd"),
            "root:x:0:0::/root:/bin/sh\nalice:x:1001:1001::/home/alice:/bin/sh\n",
        )
        .unwrap();
        fs::create_dir(dir.join("spool")).unwrap();
        let program = dir.join("tool");
        fs::write(
            &program,
            format!(
                r#"#!/usr/bin/python3
import pathlib,sys
p=pathlib.Path({:?});args=sys.argv[1:]
if args[0]=='-l':
 if (p/'readfail').exists():sys.stderr.write('fixture read failure');sys.exit(1)
 if not (p/'content').exists():sys.stderr.write('no crontab for '+args[2]+'\n');sys.exit(1)
 sys.stdout.write((p/'content').read_text());sys.exit(0)
(p/'dispatched').write_text('yes')
text=sys.stdin.read()
if (p/'writefail').exists():sys.exit(1)
(p/'content').write_text(text)
if (p/'postfail').exists():(p/'readfail').touch()
if (p/'mismatch').exists():(p/'content').write_text(text+'# external change\n')
"#,
                dir.to_string_lossy()
            ),
        )
        .unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o700)).unwrap();
        let ops = Operations {
            program,
            passwd: dir.join("passwd"),
            spool: vec![dir.join("spool")],
            authorize: Box::new(|| true),
            identity: Box::new(|_| true),
        };
        Self { dir, ops }
    }
    fn seed(&self, text: &str) {
        fs::write(self.dir.join("content"), text).unwrap();
    }
    fn revision(&self) -> String {
        let (r, c) = self.ops.snapshot("root").unwrap();
        document::revision("root", r.uid, &c)
    }
    fn append(&self, rev: &str) -> Result<String, String> {
        self.ops.change("root", rev, |s, _| {
            document::append(s, "*/5 * * * *", "/bin/true")
        })
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}
#[test]
fn physical_lines_preserve_environment_comments_tabs_specials_and_duplicate_jobs() {
    let original="# context\nSHELL=/bin/sh\n\n@reboot /bin/true\nMAILTO = \"a b\"\n*\t* * * * /bin/echo two words\n# between\n* * * * * /bin/echo two words\n";
    let rows = document::parse("root", 0, original).unwrap();
    assert_eq!(rows.iter().map(|r| r.id).collect::<Vec<_>>(), vec![4, 6, 8]);
    assert_eq!(rows[1].command, "/bin/echo two words");
    let after = document::remove("root", 0, original, 6).unwrap();
    assert_eq!(
        after,
        original.replacen("*\t* * * * /bin/echo two words\n", "", 1)
    );
    assert!(document::remove("root", 0, original, 2).is_err());
    assert!(document::remove("root", 0, original, 0).is_err());
}
#[test]
fn revision_binds_account_uid_and_all_original_bytes() {
    assert_ne!(
        document::revision("root", 0, "x\n"),
        document::revision("root", 1, "x\n")
    );
    assert_ne!(
        document::revision("root", 0, "x\n"),
        document::revision("alice", 0, "x\n")
    );
    assert_ne!(
        document::revision("root", 0, "# x\n"),
        document::revision("root", 0, "# y\n")
    );
}
#[test]
fn append_preserves_bytes_and_rejects_injected_lines_but_supports_shell_commands() {
    let text = "# retained\nSHELL=/bin/sh\n";
    assert_eq!(
        document::append(text, "0 1 * * *", "echo $(date) >> /tmp/log").unwrap(),
        format!("{text}0 1 * * * echo $(date) >> /tmp/log\n")
    );
    for bad in [
        "echo x\n* * * * * injected",
        "echo\rhidden",
        "echo\0hidden",
        "",
    ] {
        assert!(document::append(text, "* * * * *", bad).is_err());
    }
    assert!(document::append(text, "* *\n* * *", "true").is_err());
}
#[test]
fn stale_snapshot_and_read_failure_never_dispatch() {
    let f = Fixture::new();
    f.seed("# before\n");
    let rev = f.revision();
    f.seed("# externally changed\n");
    assert!(f.append(&rev).is_err());
    assert!(!f.dir.join("dispatched").exists());
    fs::write(f.dir.join("readfail"), "").unwrap();
    assert!(f.append(&rev).is_err());
    assert!(!f.dir.join("dispatched").exists());
}
#[test]
fn missing_table_can_be_created_and_post_write_is_checked() {
    let f = Fixture::new();
    let rev = f.revision();
    assert!(f.append(&rev).is_ok());
    assert_eq!(
        fs::read_to_string(f.dir.join("content")).unwrap(),
        "*/5 * * * * /bin/true\n"
    );
    assert!(f.append(&rev).is_err());
}
#[test]
fn failed_install_and_uncertain_readback_are_not_success_or_retried() {
    for marker in ["writefail", "postfail", "mismatch"] {
        let f = Fixture::new();
        f.seed("# before\n");
        let rev = f.revision();
        fs::write(f.dir.join(marker), "").unwrap();
        assert!(f.append(&rev).is_err());
        assert!(f.dir.join("dispatched").exists());
    }
}
#[test]
fn revoked_authority_and_changed_identity_cannot_write() {
    let mut f = Fixture::new();
    let rev = f.revision();
    f.ops.authorize = Box::new(|| false);
    assert!(f.append(&rev).is_err());
    assert!(!f.dir.join("dispatched").exists());
    f.ops.authorize = Box::new(|| true);
    f.ops.identity = Box::new(|_| false);
    assert!(f.append(&rev).is_err());
    assert!(!f.dir.join("dispatched").exists());
}
#[test]
fn unavailable_inventory_is_not_an_empty_success() {
    let f = Fixture::new();
    fs::remove_dir(f.dir.join("spool")).unwrap();
    assert!(f.ops.list().is_err());
}
#[test]
fn runner_bounds_time_and_output_and_closes_stdin() {
    let f = Fixture::new();
    let p = f.dir.join("hung");
    fs::write(&p, "#!/bin/sh\nsleep 10\n").unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).unwrap();
    let start = Instant::now();
    assert!(runner::run(&p, &[], None, Duration::from_millis(50)).is_err());
    assert!(start.elapsed() < Duration::from_secs(2));
    fs::write(
        &p,
        "#!/usr/bin/python3\nimport sys\nsys.stdout.write('x'*1100000)\n",
    )
    .unwrap();
    assert!(runner::run(&p, &[], None, Duration::from_secs(5)).is_err());
}
#[test]
fn shared_root_uid_is_not_editable() {
    let f = Fixture::new();
    let revision = f.revision();
    fs::write(
        f.dir.join("passwd"),
        "root:x:0:0::/root:/bin/sh\nalias:x:0:0::/root:/bin/sh\n",
    )
    .unwrap();
    assert!(f.append(&revision).is_err());
    assert!(!f.dir.join("dispatched").exists());
}
#[test]
fn update_preserves_line_position_duplicate_jobs_and_environment_scope() {
    let original = "# before\nSHELL=/bin/sh\n@reboot /bin/true\nMAILTO=first\n*\t* * * * /bin/echo duplicate\nMAILTO=second\n* * * * * /bin/echo duplicate\n# tail\n";
    let after = document::update("root", 0, original, 5, "@hourly", "/bin/echo changed").unwrap();
    assert_eq!(
        after,
        original.replacen(
            "*\t* * * * /bin/echo duplicate\n",
            "@hourly /bin/echo changed\n",
            1
        )
    );
    for id in [0, 1, 2, 4, 8, 9] {
        assert!(document::update("root", 0, original, id, "@daily", "true").is_err());
    }
    for schedule in ["@unknown", "@reboot *", "@daily\n", "* * * *"] {
        assert!(document::update("root", 0, original, 3, schedule, "true").is_err());
    }
    for command in ["  ", "true\nfalse", "true\0false"] {
        assert!(document::update("root", 0, original, 3, "@reboot", command).is_err());
    }
}
#[test]
fn stale_update_does_not_touch_replacement_at_selected_line() {
    let f = Fixture::new();
    f.seed("* * * * * /bin/true\n");
    let rev = f.revision();
    f.seed("@daily /bin/echo external\n* * * * * /bin/true\n");
    assert!(f
        .ops
        .change("root", &rev, |s, uid| document::update(
            "root",
            uid,
            s,
            1,
            "@hourly",
            "/bin/false"
        ))
        .is_err());
    assert!(!f.dir.join("dispatched").exists());
}

#[test]
fn editing_schedule_preserves_trailing_command_spaces() {
    let content = "* * * * * printf hello\\ \n";
    let row = document::parse("root", 0, content).unwrap().remove(0);
    assert_eq!(row.command, "printf hello\\ ");
    assert_eq!(
        document::update("root", 0, content, row.id, "@hourly", &row.command).unwrap(),
        "@hourly printf hello\\ \n"
    );
}
