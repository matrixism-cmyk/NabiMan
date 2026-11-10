//! The install's settings by name, typed, with their defaults. Each `*_in`
//! takes the lookup (tests); the plain form reads the environment.

use std::net::Ipv4Addr;
use std::path::PathBuf;

use super::values::{env, find_program, program_in, text_in, Lookup};

fn standard(paths: &[&str]) -> Vec<PathBuf> {
    paths.iter().map(PathBuf::from).collect()
}

/// LibreOffice's `soffice.bin` (or its `soffice` wrapper): `NABIMAN_SOFFICE`,
/// else the program where distributions and the upstream packages put it.
pub(crate) fn soffice_in(get: Lookup) -> Option<PathBuf> {
    let mut places = standard(&["/usr/lib/libreoffice/program/soffice.bin", "/usr/lib64/libreoffice/program/soffice.bin"]);
    // The upstream packages: /opt/libreoffice26.2/program/soffice.bin.
    if let Ok(entries) = std::fs::read_dir("/opt") {
        let mut found: Vec<PathBuf> = entries.flatten().map(|e| e.path())
            .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("libreoffice")))
            .map(|p| p.join("program/soffice.bin")).collect();
        found.sort();
        places.extend(found.into_iter().rev());
    }
    program_in(get, "NABIMAN_SOFFICE", &["soffice.bin"], &places)
        .or_else(|| find_program(&["soffice", "libreoffice"], &get("PATH").unwrap_or_default(), &[]))
}

pub(crate) fn soffice() -> Option<PathBuf> {
    soffice_in(&env)
}

/// rhwp, which lays out Hangul documents: `NABIMAN_RHWP`, else `PATH`.
pub(crate) fn rhwp_in(get: Lookup) -> Option<PathBuf> {
    program_in(get, "NABIMAN_RHWP", &["rhwp"], &standard(&["/usr/local/bin/rhwp", "/opt/rhwp/bin/rhwp"]))
}

pub(crate) fn rhwp() -> Option<PathBuf> {
    rhwp_in(&env)
}

/// A folder of extra fonts for previews (with a `fonts.conf` for LibreOffice):
/// `NABIMAN_PREVIEW_FONTS`, else none (the system's fonts only).
pub(crate) fn preview_fonts() -> Option<PathBuf> {
    text_in(&env, "NABIMAN_PREVIEW_FONTS").map(PathBuf::from)
}

/// Claude Code, when the voice settings do not name it: `NABIMAN_CLAUDE`.
pub(crate) fn claude_in(get: Lookup) -> Option<PathBuf> {
    text_in(get, "NABIMAN_CLAUDE").map(PathBuf::from)
}

pub(crate) fn claude() -> Option<PathBuf> {
    claude_in(&env)
}

/// guacd, for RDP consoles: `NABIMAN_GUACD_ADDR`, else 127.0.0.1:4822.
pub(crate) fn guacd_addr() -> String {
    text_in(&env, "NABIMAN_GUACD_ADDR").unwrap_or_else(|| "127.0.0.1:4822".into())
}

/// The speech service used when the dictation settings name none:
/// `NABIMAN_DICTATION_URL`, else none (dictation is not set up).
pub(crate) fn dictation_url_in(get: Lookup) -> Option<String> {
    text_in(get, "NABIMAN_DICTATION_URL")
}

pub(crate) fn dictation_url() -> Option<String> {
    dictation_url_in(&env)
}

/// The Let's Encrypt account address for new certificates:
/// `NABIMAN_LETSENCRYPT_EMAIL`, required to issue one.
pub(crate) fn letsencrypt_email_in(get: Lookup) -> Result<String, String> {
    let Some(email) = text_in(get, "NABIMAN_LETSENCRYPT_EMAIL") else {
        return Err("No Let's Encrypt e-mail address is set: set NABIMAN_LETSENCRYPT_EMAIL (or letsencrypt_email in nabiman.toml) to issue certificates".into());
    };
    let plain = email.len() <= 254
        && !email.starts_with('-')
        && email.split_once('@').is_some_and(|(user, domain)| !user.is_empty() && domain.contains('.'))
        && email.chars().all(|c| c.is_ascii_alphanumeric() || "@.+-_".contains(c));
    if plain { Ok(email) } else { Err(format!("NABIMAN_LETSENCRYPT_EMAIL is not an e-mail address: {email}")) }
}

pub(crate) fn letsencrypt_email() -> Result<String, String> {
    letsencrypt_email_in(&env)
}

/// What the DNS health check asks the local server: a name
/// (`NABIMAN_DNS_CHECK_DOMAIN`) and, optionally, the address it must answer
/// (`NABIMAN_DNS_CHECK_EXPECT`). No name: no check.
pub(crate) fn dns_check_in(get: Lookup) -> Option<(String, Option<String>)> {
    let domain = text_in(get, "NABIMAN_DNS_CHECK_DOMAIN")?;
    let plain = !domain.starts_with('-') && domain.len() <= 253
        && domain.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
    plain.then(|| (domain, text_in(get, "NABIMAN_DNS_CHECK_EXPECT")))
}

pub(crate) fn dns_check() -> Option<(String, Option<String>)> {
    dns_check_in(&env)
}

/// One MetalLB address pool, as shown in the MEC storage panel.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LbPool {
    pub name: String,
    pub ranges: Vec<(Ipv4Addr, Ipv4Addr)>,
    pub auto_assign: bool,
}

impl LbPool {
    pub fn size(&self) -> u32 {
        self.ranges.iter().map(|(a, b)| u32::from(*b) - u32::from(*a) + 1).sum()
    }
    pub fn contains(&self, ip: &str) -> bool {
        ip.parse::<Ipv4Addr>().is_ok_and(|ip| self.ranges.iter().any(|(a, b)| (*a..=*b).contains(&ip)))
    }
}

fn range(text: &str) -> Option<(Ipv4Addr, Ipv4Addr)> {
    let (a, b) = text.split_once('-').unwrap_or((text, text));
    let (a, b): (Ipv4Addr, Ipv4Addr) = (a.trim().parse().ok()?, b.trim().parse().ok()?);
    (a <= b).then_some((a, b))
}

/// `NABIMAN_MEC_LB_POOLS`: comma-separated `name=first-last [first-last…] [auto]`,
/// e.g. `tenant=10.0.0.10-10.0.0.60 auto,reserved=10.0.0.70-10.0.0.79`.
/// Items that do not read so are left out (and reported on stderr once).
pub(crate) fn mec_lb_pools_in(get: Lookup) -> Vec<LbPool> {
    super::values::list_in(get, "NABIMAN_MEC_LB_POOLS").iter().filter_map(|item| {
        let pool = item.split_once('=').and_then(|(name, rest)| {
            let mut words: Vec<&str> = rest.split_whitespace().collect();
            let auto_assign = words.last() == Some(&"auto");
            if auto_assign {
                words.pop();
            }
            let ranges: Option<Vec<_>> = words.iter().map(|w| range(w)).collect();
            let ranges = ranges.filter(|r| !r.is_empty())?;
            let name = name.trim();
            (!name.is_empty()).then(|| LbPool { name: name.into(), ranges, auto_assign })
        });
        if pool.is_none() {
            log_warn!("settings", "NABIMAN_MEC_LB_POOLS: cannot read an entry (name=first-last [auto])", entry = format!("{item:?}"));
        }
        pool
    }).collect()
}

pub(crate) fn mec_lb_pools() -> Vec<LbPool> {
    mec_lb_pools_in(&env)
}
