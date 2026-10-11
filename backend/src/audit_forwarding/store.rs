use super::Error;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::{
    fs,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::Path,
};
pub const LIMIT: i64 = 5000;
pub struct Store {
    pub db: Connection,
}
#[derive(Serialize)]
pub struct Destination {
    pub id: String,
    pub revision: String,
    pub name: String,
    pub url: String,
    pub enabled: bool,
    pub has_auth: bool,
    pub pending: i64,
    pub blocked: i64,
    pub last_error: Option<String>,
    pub acknowledged: i64,
    pub last_ack_at: Option<String>,
}
pub struct Delivery {
    pub id: String,
    pub destination: String,
    pub revision: String,
    pub url: String,
    pub auth: String,
    pub payload: String,
    pub attempts: i64,
}
fn id() -> String {
    uuid::Uuid::new_v4().to_string()
}
impl Store {
    pub fn open(path: &Path) -> Result<Self, Error> {
        let created = !path.exists();
        if created {
            let f = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW)
                .open(path)
                .map_err(|_| Error::Storage)?;
            f.sync_all().map_err(|_| Error::Storage)?;
            fs::File::open(path.parent().ok_or(Error::Storage)?)
                .and_then(|f| f.sync_all())
                .map_err(|_| Error::Storage)?;
        }
        let meta = fs::symlink_metadata(path).map_err(|_| Error::Storage)?;
        if !meta.is_file() {
            return Err(Error::Storage);
        }
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|_| Error::Storage)?;
        let db = Connection::open(path)?;
        if !created {
            let (count, version): (i64, i64) = db.query_row(
                "SELECT count(*),coalesce(min(version),0) FROM forwarding_schema",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            if count != 1 || version != 1 {
                return Err(Error::Storage);
            }
            if db.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN('destinations','deliveries','forwarding_counters')",[],|r|r.get::<_,i64>(0))?!=3 {return Err(Error::Storage)}
            if db.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
                r.get::<_, i64>(0)
            })? != 0
            {
                return Err(Error::Storage);
            }
            if db.query_row("PRAGMA quick_check", [], |r| r.get::<_, String>(0))? != "ok" {
                return Err(Error::Storage);
            }
        }
        db.execute_batch("PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON; BEGIN IMMEDIATE;
   CREATE TABLE IF NOT EXISTS forwarding_schema(version INTEGER NOT NULL);
   INSERT INTO forwarding_schema SELECT 1 WHERE NOT EXISTS(SELECT 1 FROM forwarding_schema);
   CREATE TABLE IF NOT EXISTS destinations(id TEXT PRIMARY KEY,revision TEXT NOT NULL,name TEXT NOT NULL,url TEXT NOT NULL,enabled INTEGER NOT NULL CHECK(enabled IN(0,1)),auth_enc TEXT NOT NULL,acknowledged INTEGER NOT NULL DEFAULT 0,last_ack_at TEXT,last_error TEXT);
   CREATE TABLE IF NOT EXISTS deliveries(id TEXT PRIMARY KEY,destination TEXT NOT NULL,revision TEXT NOT NULL,payload TEXT NOT NULL,probe INTEGER NOT NULL CHECK(probe IN(0,1)),state TEXT NOT NULL CHECK(state IN('pending','sending','blocked')),attempts INTEGER NOT NULL DEFAULT 0 CHECK(attempts>=0),next_attempt INTEGER NOT NULL DEFAULT 0,last_error TEXT,FOREIGN KEY(destination) REFERENCES destinations(id) ON DELETE CASCADE);
   CREATE INDEX IF NOT EXISTS forwarding_due ON deliveries(state,next_attempt);
   CREATE TABLE IF NOT EXISTS forwarding_counters(name TEXT PRIMARY KEY,value INTEGER NOT NULL);
   INSERT OR IGNORE INTO forwarding_counters VALUES('acknowledged',0),('capture_failed',0),('cancelled',0); COMMIT;")?;
        let version: i64 =
            db.query_row("SELECT version FROM forwarding_schema", [], |r| r.get(0))?;
        if version != 1 {
            return Err(Error::Storage);
        }
        if db.query_row("SELECT count(*) FROM deliveries q LEFT JOIN destinations d ON d.id=q.destination AND d.revision=q.revision WHERE d.id IS NULL",[],|r|r.get::<_,i64>(0))?!=0 {return Err(Error::Storage)}
        db.execute("UPDATE deliveries SET state='pending',next_attempt=0,last_error='interrupted_possible_duplicate' WHERE state='sending'",[])?;
        Ok(Self { db })
    }
    pub fn list(&self) -> Result<Vec<Destination>, Error> {
        let mut stmt=self.db.prepare("SELECT id,revision,name,url,enabled,auth_enc!='', (SELECT count(*) FROM deliveries WHERE destination=d.id AND state!='blocked'), (SELECT count(*) FROM deliveries WHERE destination=d.id AND state='blocked'), d.last_error,d.acknowledged,d.last_ack_at FROM destinations d ORDER BY rowid")?;
        let rows = stmt.query_map([], |r| {
            Ok(Destination {
                id: r.get(0)?,
                revision: r.get(1)?,
                name: r.get(2)?,
                url: r.get(3)?,
                enabled: r.get(4)?,
                has_auth: r.get(5)?,
                pending: r.get(6)?,
                blocked: r.get(7)?,
                last_error: r.get(8)?,
                acknowledged: r.get(9)?,
                last_ack_at: r.get(10)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
    pub fn add(&mut self, name: &str, url: &str, auth: &str) -> Result<String, Error> {
        let tx = self.db.transaction()?;
        if tx.query_row("SELECT count(*) FROM destinations", [], |r| {
            r.get::<_, i64>(0)
        })? >= 8
        {
            return Err(Error::Limit);
        }
        let key = id();
        tx.execute(
            "INSERT INTO destinations(id,revision,name,url,enabled,auth_enc) VALUES(?1,?2,?3,?4,0,?5)",
            params![key, id(), name, url, auth],
        )?;
        tx.commit()?;
        Ok(key)
    }
    pub fn change(&mut self, key: &str, revision: &str, action: &str) -> Result<(), Error> {
        let tx = self.db.transaction()?;
        let current: Option<String> = tx
            .query_row(
                "SELECT revision FROM destinations WHERE id=?1",
                [key],
                |r| r.get(0),
            )
            .optional()?;
        if current.as_deref() != Some(revision) {
            return Err(Error::Conflict);
        }
        if action == "retry" {
            tx.execute("UPDATE deliveries SET state='pending',attempts=0,next_attempt=0,last_error=NULL WHERE destination=?1 AND state='blocked'",[key])?;
        } else {
            let count = tx.execute("DELETE FROM deliveries WHERE destination=?1", [key])?;
            tx.execute(
                "UPDATE forwarding_counters SET value=value+?1 WHERE name='cancelled'",
                [count],
            )?;
            match action {
                "enable" | "disable" => {
                    tx.execute(
                        "UPDATE destinations SET revision=?1,enabled=?2 WHERE id=?3",
                        params![id(), action == "enable", key],
                    )?;
                }
                "delete" => {
                    tx.execute("DELETE FROM destinations WHERE id=?1", [key])?;
                }
                _ => return Err(Error::Invalid),
            }
        }
        tx.commit()?;
        Ok(())
    }
    pub fn enqueue(
        &mut self,
        entry: &crate::audit::AuditEntry,
        probe: Option<(&str, &str)>,
    ) -> Result<usize, Error> {
        let tx = self.db.transaction()?;
        let targets: Vec<(String, String)> = {
            let mut statement =
                tx.prepare("SELECT id,revision FROM destinations WHERE enabled=1 OR id=?1")?;
            let rows = statement.query_map([probe.map(|p| p.0).unwrap_or("")], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?;
            rows.collect::<Result<Vec<(String, String)>, _>>()?
        }
        .into_iter()
        .filter(|(key, _)| probe.map_or(true, |p| p.0 == key))
        .collect();
        if let Some((key, rev)) = probe {
            if !targets.iter().any(|(k, r)| k == key && r == rev) {
                return Err(Error::Conflict);
            }
        }
        if targets.is_empty() {
            return Ok(0);
        }
        let event = id();
        let payload=serde_json::to_string(&serde_json::json!({"schema_version":1,"event_id":event,"probe":probe.is_some(),"entry":entry})).map_err(|_|Error::Invalid)?;
        let (count, bytes): (i64, i64) = tx.query_row(
            "SELECT count(*),coalesce(sum(length(CAST(payload AS BLOB))),0) FROM deliveries",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        if count + targets.len() as i64 > LIMIT
            || payload.len() > 16384
            || bytes + payload.len() as i64 * targets.len() as i64 > 32 * 1024 * 1024
        {
            tx.execute(
                "UPDATE forwarding_counters SET value=value+1 WHERE name='capture_failed'",
                [],
            )?;
            tx.commit()?;
            return Err(Error::Limit);
        }
        for (key, revision) in &targets {
            tx.execute("INSERT INTO deliveries(id,destination,revision,payload,probe,state) VALUES(?1,?2,?3,?4,?5,'pending')",params![id(),key,revision,payload,probe.is_some()])?;
        }
        tx.commit()?;
        Ok(targets.len())
    }
    pub fn claim(&mut self, now: i64) -> Result<Option<Delivery>, Error> {
        let tx = self.db.transaction()?;
        let row=tx.query_row("SELECT q.id,q.destination,q.revision,d.url,d.auth_enc,q.payload,q.attempts+1 FROM deliveries q JOIN destinations d ON d.id=q.destination AND d.revision=q.revision WHERE q.state='pending' AND q.next_attempt<=?1 AND (d.enabled=1 OR q.probe=1) ORDER BY q.rowid LIMIT 1",[now],|r|Ok(Delivery{id:r.get(0)?,destination:r.get(1)?,revision:r.get(2)?,url:r.get(3)?,auth:r.get(4)?,payload:r.get(5)?,attempts:r.get(6)?})).optional()?;
        if let Some(row) = &row {
            tx.execute(
                "UPDATE deliveries SET state='sending',attempts=?1 WHERE id=?2",
                params![row.attempts, row.id],
            )?;
        }
        tx.commit()?;
        Ok(row)
    }
    pub fn finish(&mut self, row: &Delivery, error: Option<&str>, now: i64) -> Result<(), Error> {
        let tx = self.db.transaction()?;
        if let Some(error) = error {
            let changed=tx.execute("UPDATE deliveries SET state=?1,next_attempt=?2,last_error=?3 WHERE id=?4 AND revision=?5",params![if row.attempts>=8{"blocked"}else{"pending"},now+(5*2i64.pow(row.attempts.min(9) as u32)).min(3600),error,row.id,row.revision])?;
            if changed > 0 {
                tx.execute(
                    "UPDATE destinations SET last_error=?1 WHERE id=?2 AND revision=?3",
                    params![error, row.destination, row.revision],
                )?;
            }
        } else {
            let count = tx.execute(
                "DELETE FROM deliveries WHERE id=?1 AND revision=?2",
                params![row.id, row.revision],
            )?;
            if count > 0 {
                tx.execute("UPDATE destinations SET acknowledged=acknowledged+1,last_ack_at=?1,last_error=NULL WHERE id=?2 AND revision=?3",params![chrono::DateTime::from_timestamp(now,0).ok_or(Error::Invalid)?.to_rfc3339(),row.destination,row.revision])?;
            }
            tx.execute(
                "UPDATE forwarding_counters SET value=value+?1 WHERE name='acknowledged'",
                [count],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn counters(&self) -> Result<std::collections::BTreeMap<String, i64>, Error> {
        let mut stmt = self
            .db
            .prepare("SELECT name,value FROM forwarding_counters")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}
