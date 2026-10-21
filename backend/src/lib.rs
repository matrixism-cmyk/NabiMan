//! Process boundaries shared by the API and the privileged helper.
pub mod privileged;

pub mod backup_crypto;
pub mod recovery_bundle;

/// Where NabiMan keeps its data: `$NABIMAN_DATA_DIR`, else /var/lib/nabiman.
/// The one place this is decided. Read on every call, so a test (or a
/// process given another directory) sees its own.
pub fn data_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var("NABIMAN_DATA_DIR").unwrap_or_else(|_| "/var/lib/nabiman".into()))
}
