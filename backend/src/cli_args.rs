//! What may be handed to a command-line tool as a value it did not choose:
//! a login name, a host, a database name. Anything starting with `-` would
//! be read as an option (`-oProxyCommand=…`, `--init-command=…`), so it is
//! refused here, and callers also end the options with `--` where the tool
//! understands it.

/// Not empty and not something a tool would parse as an option.
pub(crate) fn not_option(value: &str) -> bool {
    !value.is_empty() && !value.starts_with('-')
}

/// A database name as `mysqldump`, `mysql`, `pg_dump` and `psql` are given
/// it: letters, digits, `_`, `$` and `-` (never first), at most 64
/// characters (MySQL's limit; PostgreSQL's is 63 bytes). No quote, space,
/// slash or `;` can reach a client or a shell.
pub(crate) fn database_name(name: &str) -> bool {
    not_option(name)
        && name.len() <= 64
        && name.chars().all(|c| c.is_alphanumeric() || "_$-".contains(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leading_dashes_are_options_not_values() {
        assert!(not_option("root") && not_option("a-b"));
        for v in ["", "-", "-oProxyCommand=x", "--help"] {
            assert!(!not_option(v), "{v}");
        }
    }

    #[test]
    fn database_names_carry_no_option_quote_or_path() {
        for ok in ["app", "app_db", "wp-site", "db$1", "한글db"] {
            assert!(database_name(ok), "{ok}");
        }
        for bad in [
            "", "-e", "--init-command=DROP", "a'b", "a\"b", "a b", "a;b", "../x", "a/b", "a`b`", "$(id)x(",
            &"x".repeat(65),
        ] {
            assert!(!database_name(bad), "{bad}");
        }
    }
}
