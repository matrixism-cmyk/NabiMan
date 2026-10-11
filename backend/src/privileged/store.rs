use super::{
    policy::{validate_parent, Policy},
    protocol::{Receipt, Request, ServiceAction, State},
};
use rusqlite::{Connection, OptionalExtension};
use std::{
    fs, io,
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, OpenOptionsExt},
    },
    path::Path,
};

pub(crate) struct Store {
    connection: Connection,
    _lock: Option<fs::File>,
}
fn error(error: impl std::fmt::Display) -> io::Error {
    io::Error::other(error.to_string())
}
impl Store {
    pub(crate) fn open(path: &Path) -> io::Result<Self> {
        validate_parent(path)?;
        let lock_path = path.with_extension("lock");
        let established = lock_path.exists();
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(&lock_path)?;
        let metadata = lock.metadata()?;
        if !metadata.is_file() || metadata.uid() != 0 || metadata.mode() & 0o077 != 0 {
            return Err(error("Unsafe helper ledger lock"));
        }
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(io::Error::last_os_error());
        }
        if established && !path.exists() {
            return Err(error("Established helper ledger is missing"));
        }
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.uid() != 0 || metadata.mode() & 0o077 != 0 {
            return Err(error("Unsafe helper ledger"));
        }
        file.sync_all()?;
        lock.sync_all()?;
        fs::File::open(path.parent().unwrap())?.sync_all()?;
        let mut store = Self::initialize(Connection::open(path).map_err(error)?, Some(lock))?;
        store.recover()?;
        Ok(store)
    }
    fn initialize(connection: Connection, lock: Option<fs::File>) -> io::Result<Self> {
        connection
            .busy_timeout(std::time::Duration::from_secs(1))
            .map_err(error)?;
        connection
            .execute_batch("PRAGMA journal_mode=DELETE; PRAGMA synchronous=EXTRA;")
            .map_err(error)?;
        let synchronous: i64 = connection
            .query_row("PRAGMA synchronous", [], |r| r.get(0))
            .map_err(error)?;
        let journal: String = connection
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .map_err(error)?;
        if synchronous != 3 || (lock.is_some() && journal != "delete") {
            return Err(error("Unsafe helper ledger durability settings"));
        }
        let check: String = connection
            .query_row("PRAGMA quick_check", [], |r| r.get(0))
            .map_err(error)?;
        if check != "ok" {
            return Err(error("Corrupt helper ledger"));
        }
        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(error)?;
        if version == 0 {
            let tables: i64 = connection
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type='table'",
                    [],
                    |r| r.get(0),
                )
                .map_err(error)?;
            if tables != 0 {
                return Err(error("Unrecognized helper ledger schema"));
            }
            connection.execute_batch("BEGIN IMMEDIATE; CREATE TABLE helper_operations(id TEXT PRIMARY KEY, actor_id TEXT NOT NULL, unit TEXT NOT NULL, state TEXT NOT NULL, request TEXT NOT NULL, receipt TEXT NOT NULL); CREATE INDEX unresolved_helper_unit ON helper_operations(unit,state); CREATE TABLE helper_policies(hash TEXT PRIMARY KEY, snapshot TEXT NOT NULL); PRAGMA user_version=1; COMMIT;").map_err(error)?;
        } else if version != 1 {
            return Err(error("Unsupported helper ledger schema"));
        }
        connection
            .prepare("SELECT hash,snapshot FROM helper_policies LIMIT 0")
            .map_err(error)?;
        Ok(Self {
            connection,
            _lock: lock,
        })
    }
    #[cfg(test)]
    pub(crate) fn memory() -> Self {
        Self::initialize(Connection::open_in_memory().unwrap(), None).unwrap()
    }
    #[cfg(test)]
    pub(crate) fn inject_failure(&self, confirmation: bool) {
        let action = if confirmation { "UPDATE" } else { "INSERT" };
        self.connection.execute_batch(&format!("CREATE TRIGGER fault BEFORE {action} ON helper_operations BEGIN SELECT RAISE(ABORT,'fixture failure'); END;")).unwrap();
    }
    #[cfg(test)]
    pub(crate) fn clear_failure(&self) {
        self.connection.execute_batch("DROP TRIGGER fault").unwrap();
    }
    pub(crate) fn get(&self, id: &str) -> io::Result<Option<Receipt>> {
        let json: Option<String> = self
            .connection
            .query_row(
                "SELECT receipt FROM helper_operations WHERE id=?1",
                [id],
                |r| r.get(0),
            )
            .optional()
            .map_err(error)?;
        json.map(|json| serde_json::from_str(&json).map_err(error))
            .transpose()
    }
    pub(crate) fn blocked(&self, unit: &str) -> io::Result<Option<Receipt>> {
        let json:Option<String>=self.connection.query_row("SELECT receipt FROM helper_operations WHERE unit=?1 AND state IN ('running','unknown_result') LIMIT 1",[unit],|r|r.get(0)).optional().map_err(error)?;
        json.map(|json| serde_json::from_str(&json).map_err(error))
            .transpose()
    }
    pub(crate) fn begin(
        &mut self,
        request: &Request,
        unit: &str,
        action: ServiceAction,
        policy: &Policy,
    ) -> io::Result<bool> {
        let transaction = self.connection.transaction().map_err(error)?;
        let existing: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM helper_operations WHERE id=?1)",
                [&request.request_id],
                |r| r.get(0),
            )
            .map_err(error)?;
        if existing {
            transaction.commit().map_err(error)?;
            return Ok(false);
        }
        let count: i64 = transaction
            .query_row("SELECT count(*) FROM helper_operations", [], |r| r.get(0))
            .map_err(error)?;
        if count >= 100000 {
            return Err(error(
                "Helper ledger capacity requires reviewed maintenance",
            ));
        }
        let now = chrono::Utc::now().to_rfc3339();
        let policy_hash = policy.version();
        let receipt = Receipt {
            policy_hash: policy_hash.clone(),
            operation_id: request.request_id.clone(),
            actor_id: request.actor_id.clone(),
            unit: unit.into(),
            action,
            state: State::Running,
            exit_code: None,
            created_at: now.clone(),
            updated_at: now,
        };
        transaction
            .execute(
                "INSERT OR IGNORE INTO helper_policies(hash,snapshot) VALUES (?1,?2)",
                rusqlite::params![policy_hash, serde_json::to_string(policy).map_err(error)?],
            )
            .map_err(error)?;
        let changed=transaction.execute("INSERT OR IGNORE INTO helper_operations(id,actor_id,unit,state,request,receipt) VALUES (?1,?2,?3,?4,?5,?6)",rusqlite::params![request.request_id,request.actor_id,unit,"running",serde_json::to_string(request).map_err(error)?,serde_json::to_string(&receipt).map_err(error)?]).map_err(error)?;
        transaction.commit().map_err(error)?;
        Ok(changed == 1)
    }
    pub(crate) fn finish(
        &mut self,
        id: &str,
        state: State,
        exit_code: Option<i32>,
    ) -> io::Result<Receipt> {
        let mut receipt = self
            .get(id)?
            .ok_or_else(|| error("Missing helper intent"))?;
        receipt.state = state;
        receipt.exit_code = exit_code;
        receipt.updated_at = chrono::Utc::now().to_rfc3339();
        if self
            .connection
            .execute(
                "UPDATE helper_operations SET receipt=?1,state=?3 WHERE id=?2",
                rusqlite::params![
                    serde_json::to_string(&receipt).map_err(error)?,
                    id,
                    state.as_str()
                ],
            )
            .map_err(error)?
            != 1
        {
            return Err(error("Missing helper intent"));
        }
        Ok(receipt)
    }
    pub(crate) fn recover(&mut self) -> io::Result<()> {
        let count: i64 = self
            .connection
            .query_row("SELECT count(*) FROM helper_operations", [], |r| r.get(0))
            .map_err(error)?;
        if count > 100000 {
            return Err(error("Oversized helper ledger"));
        }
        let mut running = Vec::new();
        {
            let mut query = self
                .connection
                .prepare("SELECT id,actor_id,unit,state,request,receipt FROM helper_operations")
                .map_err(error)?;
            let rows = query
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, String>(5)?,
                    ))
                })
                .map_err(error)?;
            for row in rows {
                let (id, actor, unit, state, intent, json) = row.map_err(error)?;
                let receipt: Receipt = serde_json::from_str(&json).map_err(error)?;
                let request: Request = serde_json::from_str(&intent).map_err(error)?;
                let exact = matches!(request.operation,super::protocol::Operation::Execute{unit:ref target,action} if target==&unit && action==receipt.action);
                let snapshot: String = self
                    .connection
                    .query_row(
                        "SELECT snapshot FROM helper_policies WHERE hash=?1",
                        [&receipt.policy_hash],
                        |r| r.get(0),
                    )
                    .map_err(error)?;
                let policy: Policy = serde_json::from_str(&snapshot).map_err(error)?;
                policy.validate()?;
                if policy.version() != receipt.policy_hash
                    || !policy.permits(policy.api_uid, &unit, receipt.action)
                {
                    return Err(error("Inconsistent helper approval snapshot"));
                }
                if !exact
                    || request.version != super::protocol::VERSION
                    || request.request_id != id
                    || request.actor_id != actor
                    || receipt.actor_id != actor
                    || receipt.operation_id != id
                    || receipt.unit != unit
                    || receipt.state.as_str() != state
                    || !super::protocol::is_id(&id)
                    || super::policy::canonical_unit(&unit).as_deref() != Some(&unit)
                {
                    return Err(error("Inconsistent helper ledger"));
                }
                if receipt.state == State::Running {
                    running.push(id);
                }
            }
        }
        for id in running {
            self.finish(&id, State::UnknownResult, None)?;
        }
        Ok(())
    }
}
