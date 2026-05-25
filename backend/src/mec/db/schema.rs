use rusqlite::Connection;

pub fn initialize(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(SCHEMA_SQL)?;
    Ok(())
}

const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS tenant_profiles (
    id                          TEXT PRIMARY KEY,
    display_name                TEXT NOT NULL,
    task_name                   TEXT,
    contact_email               TEXT,
    contact_manager             TEXT,
    contact_phone               TEXT,
    namespace                   TEXT NOT NULL,
    rancher_project_id          TEXT,
    rancher_user_id             TEXT,
    allocation_json             TEXT NOT NULL,
    quota_json                  TEXT NOT NULL,
    starter_kit_json            TEXT,
    starter_ssh_password_hash   TEXT,
    harbor_project_created      INTEGER DEFAULT 0,
    status                      TEXT NOT NULL,
    created_at                  TEXT NOT NULL,
    created_by                  TEXT,
    updated_at                  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_tenant_profiles_created_at ON tenant_profiles(created_at);

CREATE TABLE IF NOT EXISTS jobs (
    id                  TEXT PRIMARY KEY,
    kind                TEXT NOT NULL,
    status              TEXT NOT NULL,
    spec                TEXT,
    steps               TEXT,
    result              TEXT,
    error               TEXT,
    created_by          TEXT,
    started_at          TEXT NOT NULL,
    completed_at        TEXT
);
CREATE INDEX IF NOT EXISTS idx_jobs_status ON jobs(status);
CREATE INDEX IF NOT EXISTS idx_jobs_started_at ON jobs(started_at);
CREATE INDEX IF NOT EXISTS idx_jobs_created_by ON jobs(created_by);

CREATE TABLE IF NOT EXISTS audit_logs (
    id              TEXT PRIMARY KEY,
    timestamp       TEXT NOT NULL,
    user            TEXT NOT NULL,
    action          TEXT NOT NULL,
    resource_type   TEXT NOT NULL,
    resource_id     TEXT NOT NULL,
    status          TEXT NOT NULL,
    duration_ms     INTEGER,
    input           TEXT,
    output          TEXT,
    error           TEXT,
    operations      TEXT,
    source_ip       TEXT,
    user_agent      TEXT
);
CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON audit_logs(timestamp);
CREATE INDEX IF NOT EXISTS idx_audit_user ON audit_logs(user);
CREATE INDEX IF NOT EXISTS idx_audit_action ON audit_logs(action);
CREATE INDEX IF NOT EXISTS idx_audit_resource ON audit_logs(resource_type, resource_id);

CREATE TABLE IF NOT EXISTS generated_docs (
    id                TEXT PRIMARY KEY,
    template          TEXT NOT NULL,
    format            TEXT NOT NULL,
    filename          TEXT NOT NULL,
    file_path         TEXT NOT NULL,
    file_size_bytes   INTEGER,
    related_tenants   TEXT,
    generated_by      TEXT,
    generated_at      TEXT NOT NULL,
    expires_at        TEXT,
    download_count    INTEGER DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_docs_template ON generated_docs(template);
CREATE INDEX IF NOT EXISTS idx_docs_generated_at ON generated_docs(generated_at);

CREATE TABLE IF NOT EXISTS firewall_cache (
    cache_key   TEXT PRIMARY KEY,
    data        TEXT NOT NULL,
    cached_at   TEXT NOT NULL,
    expires_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS mec_config (
    key           TEXT PRIMARY KEY,
    value         TEXT NOT NULL,
    value_type    TEXT,
    description   TEXT,
    updated_at    TEXT NOT NULL,
    updated_by    TEXT
);
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_all_tables() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        let expected = [
            "tenant_profiles",
            "jobs",
            "audit_logs",
            "generated_docs",
            "firewall_cache",
            "mec_config",
        ];
        for name in expected {
            let n: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [name],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 1, "table {} missing", name);
        }
    }

    #[test]
    fn idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        initialize(&conn).unwrap();
    }
}
