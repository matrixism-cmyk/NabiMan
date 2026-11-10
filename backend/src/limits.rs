//! Server-wide caps an operator can change through the environment. Each is
//! read once; unset, unreadable or zero means the default.

use std::sync::OnceLock;

fn read(name: &str, default: u64, cell: &OnceLock<u64>) -> u64 {
    *cell.get_or_init(|| match std::env::var(name).ok().and_then(|v| v.trim().parse::<u64>().ok()) {
        Some(n) if n > 0 => n,
        _ => default,
    })
}

/// `NABIMAN_UPLOAD_MAX_MB`: the largest single file anyone may upload through
/// the file panel (SFTP/FTP), in MiB. Default 10240 (10 GiB). A member's own,
/// smaller limit still applies.
pub const UPLOAD_MAX_MB_DEFAULT: u64 = 10 * 1024;
pub fn upload_max_bytes() -> u64 {
    static CELL: OnceLock<u64> = OnceLock::new();
    read("NABIMAN_UPLOAD_MAX_MB", UPLOAD_MAX_MB_DEFAULT, &CELL).saturating_mul(1024 * 1024)
}

/// `NABIMAN_MAX_SESSIONS_PER_USER`: terminal sessions one account may have at
/// once. Default 100. A member's own, smaller limit still applies.
pub const SESSIONS_PER_USER_DEFAULT: u64 = 100;
pub fn sessions_per_user() -> usize {
    static CELL: OnceLock<u64> = OnceLock::new();
    read("NABIMAN_MAX_SESSIONS_PER_USER", SESSIONS_PER_USER_DEFAULT, &CELL) as usize
}

/// Why one more session is refused to an account that has `running`.
pub fn session_limit(running: usize) -> Option<String> {
    let max = sessions_per_user();
    (running >= max).then(|| format!("세션은 계정마다 동시에 {max}개까지 열 수 있습니다. 쓰지 않는 세션을 종료한 뒤 다시 여세요."))
}

/// `NABIMAN_SFTP_MAX_CONNECTIONS`: open SFTP connections kept for the file
/// panel, across all operators. Default 256; past it the least recently used
/// connection is closed.
pub const SFTP_CONNECTIONS_DEFAULT: u64 = 256;
pub fn sftp_connections() -> usize {
    static CELL: OnceLock<u64> = OnceLock::new();
    read("NABIMAN_SFTP_MAX_CONNECTIONS", SFTP_CONNECTIONS_DEFAULT, &CELL) as usize
}

#[cfg(test)]
mod tests {
    #[test]
    fn unset_unreadable_or_zero_is_the_default() {
        let cell = std::sync::OnceLock::new();
        std::env::set_var("NABIMAN_LIMITS_TEST_A", "0");
        assert_eq!(super::read("NABIMAN_LIMITS_TEST_A", 7, &cell), 7);
        let cell = std::sync::OnceLock::new();
        std::env::set_var("NABIMAN_LIMITS_TEST_B", "lots");
        assert_eq!(super::read("NABIMAN_LIMITS_TEST_B", 7, &cell), 7);
        let cell = std::sync::OnceLock::new();
        std::env::set_var("NABIMAN_LIMITS_TEST_C", " 12 ");
        assert_eq!(super::read("NABIMAN_LIMITS_TEST_C", 7, &cell), 12);
        assert_eq!(super::session_limit(super::sessions_per_user() - 1), None);
        assert!(super::session_limit(super::sessions_per_user()).is_some());
    }
}
