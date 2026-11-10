//! Every setting NabiMan reads, by its environment name: what the settings
//! file may contain without a warning. `docs/operations/configuration.md`
//! describes each one; a test checks that the two and the code agree.

pub(crate) const KNOWN: &[&str] = &[
    // Where things are
    "NABIMAN_CONFIG",
    "NABIMAN_DATA_DIR",
    "NABIMAN_STATIC",
    "NABIMAN_PORT",
    "NABIMAN_LISTEN",
    "NABIMAN_RUN_MODE",
    "NABIMAN_SECRET_KEY_FILE",
    "NABIMAN_HELPER_SOCKET",
    "NABIMAN_CLAUDE_SOCKET",
    "NABIMAN_LICENSE_PUBLIC_KEYS_FILE",
    // Sign-in and the web
    "NABIMAN_PASSWORD",
    "NABIMAN_API_TOKENS_DISABLED",
    "NABIMAN_CORS_ORIGINS",
    "NABIMAN_TRUSTED_PROXIES",
    "NABIMAN_CSP",
    "NABIMAN_HSTS",
    "NABIMAN_PRODUCT_NAME",
    "NABIMAN_FEEDBACK_ORIGIN",
    "NABIMAN_FEEDBACK_SITE",
    "NABIMAN_FEEDBACK_SCRIPT",
    "NABIMAN_NOC_CONTACT",
    // Limits
    "NABIMAN_UPLOAD_MAX_MB",
    "NABIMAN_MAX_SESSIONS_PER_USER",
    "NABIMAN_SFTP_MAX_CONNECTIONS",
    // Backup and audit
    "NABIMAN_DEFAULT_BACKUP_SCHEDULE",
    "NABIMAN_DEFAULT_BACKUP_KEEP",
    "NABIMAN_AUDIT_CA_FILE",
    "NABIMAN_AUDIT_FORWARDING_DISABLED",
    // External programs and services
    "NABIMAN_SOFFICE",
    "NABIMAN_RHWP",
    "NABIMAN_PREVIEW_FONTS",
    "NABIMAN_CLAUDE",
    "NABIMAN_GUACD_ADDR",
    "NABIMAN_SERIAL_GROUP",
    "NABIMAN_DICTATION_URL",
    "NABIMAN_LETSENCRYPT_EMAIL",
    "NABIMAN_DNS_CHECK_DOMAIN",
    "NABIMAN_DNS_CHECK_EXPECT",
    "NABIMAN_DNS_DB_DEFAULTS_FILE",
    "NABIMAN_DNS_DB_PASSWORD",
    // MEC
    "NABIMAN_MEC_MODE",
    "NABIMAN_MEC_DB",
    "NABIMAN_MEC_DOCS",
    "NABIMAN_MEC_READ_ONLY",
    "NABIMAN_MEC_SKIP_CONFIRM",
    "NABIMAN_MEC_NAMESPACE",
    "NABIMAN_MEC_KUBECONFIG",
    "NABIMAN_MEC_LB_POOLS",
    "NABIMAN_MEC_DCGM_URL",
    "NABIMAN_MEC_RANCHER_URL",
    "NABIMAN_MEC_RANCHER_TOKEN",
    "NABIMAN_MEC_RANCHER_CLUSTER",
    "NABIMAN_MEC_RANCHER_INSECURE",
    "NABIMAN_MEC_HARBOR_URL",
    "NABIMAN_MEC_HARBOR_USER",
    "NABIMAN_MEC_HARBOR_PASSWORD",
    "NABIMAN_MEC_HARBOR_INSECURE",
    "NABIMAN_MEC_AXGATE_HOST",
    "NABIMAN_MEC_AXGATE_PORT",
    "NABIMAN_MEC_AXGATE_USER",
    "NABIMAN_MEC_AXGATE_PASSWORD",
    "NABIMAN_MEC_AXGATE_HOST_KEY_SHA256",
    "NABIMAN_MEC_AXGATE_TIMEOUT",
    "NABIMAN_MEC_AXGATE_VPN",
    "NABIMAN_MEC_AXGATE_LEGACY_UNTIL",
    "NABIMAN_MEC_AXGATE_DEBUG",
    "NABIMAN_MEC_AXGATE_DUMP_CONFIG",
];

/// Names in the code that are not settings: build stamps, test switches,
/// and values NabiMan hands to programs it starts.
#[cfg(test)]
pub(crate) const NOT_SETTINGS: &[&str] = &[
    "NABIMAN_GIT_COMMIT",
    "NABIMAN_OFFSETS",
    "NABIMAN_WRITE_ROUTE_DOC",
    "NABIMAN_RESTORE_TEST_PHASE",
];

/// Settings that may not come from the file: which file to read, and, in
/// the file inside the data directory, the data directory itself.
pub(crate) fn not_from_file(name: &str, file_in_data_dir: bool) -> bool {
    name == "NABIMAN_CONFIG" || (file_in_data_dir && name == "NABIMAN_DATA_DIR")
}
