//! Database dump and restore as a pipe through gzip, without building a shell
//! line from the database name: the script is fixed, and the file and the
//! client's argv reach it as positional parameters, which the shell never
//! re-parses. Either end failing fails the whole run (POSIX sh has no
//! `pipefail`, so the producer's status is passed out on fd 3), and a dump
//! that failed leaves no half-written archive behind.

use std::path::Path;
use std::process::Command;

/// `$1` is the archive to write; the rest is the dump client's argv.
const DUMP: &str = r#"out=$1; shift
st=$( { { "$@"; echo $? >&3; } | gzip > "$out"; } 3>&1 ) || { rm -f -- "$out"; exit 1; }
[ "$st" = 0 ] || { rm -f -- "$out"; exit 1; }"#;

/// `$1` is the archive to read; the rest is the restore client's argv.
const RESTORE: &str = r#"src=$1; shift
st=$( { { gunzip -c -- "$src"; echo $? >&3; } | "$@" > /dev/null; } 3>&1 ) || exit 1
[ "$st" = 0 ] || { echo "The backup file could not be decompressed" >&2; exit 1; }"#;

fn dump_client(engine: &str, db: &str) -> Option<Vec<String>> {
    let argv: &[&str] = match engine {
        "mysql" => &["mysqldump", "--"],
        "postgresql" => &["sudo", "-u", "postgres", "--", "pg_dump", "--"],
        _ => return None,
    };
    Some(argv.iter().map(|a| a.to_string()).chain([db.to_string()]).collect())
}

fn restore_client(engine: &str, db: &str) -> Option<Vec<String>> {
    let argv: &[&str] = match engine {
        "mysql" => &["mysql", "--"],
        "postgresql" => &["sudo", "-u", "postgres", "--", "psql", "--"],
        _ => return None,
    };
    Some(argv.iter().map(|a| a.to_string()).chain([db.to_string()]).collect())
}

fn piped(script: &str, file: &Path, client: Vec<String>) -> Command {
    let mut cmd = Command::new("sh");
    cmd.args(["-c", script, "sh"]).arg(file).args(client);
    cmd
}

/// Dumps `db` into `dest` (gzip). `None` for an engine it does not know.
pub(crate) fn dump_command(engine: &str, db: &str, dest: &Path) -> Option<Command> {
    Some(piped(DUMP, dest, dump_client(engine, db)?))
}

/// Feeds the gzip archive `src` into `db`. `None` for an unknown engine.
pub(crate) fn restore_command(engine: &str, db: &str, src: &Path) -> Option<Command> {
    Some(piped(RESTORE, src, restore_client(engine, db)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn scratch(what: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("nabiman-dbpipe-{what}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn args(cmd: &Command) -> Vec<String> {
        cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect()
    }

    #[test]
    fn the_database_name_is_one_argument_after_the_options() {
        let evil = "x'; touch /tmp/pwned; '";
        for engine in ["mysql", "postgresql"] {
            for cmd in [dump_command(engine, evil, Path::new("/d/f.gz")).unwrap(), restore_command(engine, evil, Path::new("/d/f.gz")).unwrap()] {
                let a = args(&cmd);
                assert_eq!(a.last().map(String::as_str), Some(evil));
                assert_eq!(a[a.len() - 2], "--", "{a:?}");
                assert!(!a[1].contains(evil), "the script never carries the name");
            }
        }
        assert!(dump_command("oracle", "db", Path::new("/f")).is_none());
    }

    #[test]
    fn a_dump_is_gzipped_and_a_failed_one_leaves_nothing() {
        let dir = scratch("dump");
        let out = dir.join("it's.sql.gz");
        let ok = piped(DUMP, &out, vec!["printf".into(), "%s".into(), "a'b $(id)".into()]).status().unwrap();
        assert!(ok.success());
        let mut text = String::new();
        flate2::read::GzDecoder::new(std::fs::File::open(&out).unwrap()).read_to_string(&mut text).unwrap();
        assert_eq!(text, "a'b $(id)");

        let failed = dir.join("failed.sql.gz");
        let st = piped(DUMP, &failed, vec!["sh".into(), "-c".into(), "echo partial; exit 3".into()]).status().unwrap();
        assert!(!st.success());
        assert!(!failed.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_restore_fails_when_either_end_does() {
        let dir = scratch("restore");
        let good = dir.join("good.gz");
        assert!(piped(DUMP, &good, vec!["printf".into(), "x".into()]).status().unwrap().success());
        assert!(piped(RESTORE, &good, vec!["cat".into()]).status().unwrap().success());
        assert!(!piped(RESTORE, &good, vec!["false".into()]).status().unwrap().success());
        let corrupt = dir.join("corrupt.gz");
        std::fs::write(&corrupt, b"not gzip").unwrap();
        assert!(!piped(RESTORE, &corrupt, vec!["cat".into()]).stderr(std::process::Stdio::null()).status().unwrap().success());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
