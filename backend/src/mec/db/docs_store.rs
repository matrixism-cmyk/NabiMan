use super::DbHandle;
use chrono::{DateTime, Utc};
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedDoc {
    pub id: String,
    pub template: String,
    pub format: String,
    pub filename: String,
    pub file_path: String,
    pub file_size_bytes: u64,
    pub related_tenants: Vec<String>,
    pub generated_by: Option<String>,
    pub generated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub download_count: u32,
}

pub struct DocsStore {
    db: DbHandle,
}

impl DocsStore {
    pub fn new(db: DbHandle) -> Self {
        Self { db }
    }

    pub fn insert(&self, doc: &GeneratedDoc) -> rusqlite::Result<()> {
        let conn = self.db.lock().unwrap();
        conn.execute(
            "INSERT INTO generated_docs
                (id, template, format, filename, file_path, file_size_bytes,
                 related_tenants, generated_by, generated_at, expires_at, download_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                doc.id,
                doc.template,
                doc.format,
                doc.filename,
                doc.file_path,
                doc.file_size_bytes as i64,
                serde_json::to_string(&doc.related_tenants).unwrap_or_default(),
                doc.generated_by,
                doc.generated_at.to_rfc3339(),
                doc.expires_at.map(|d| d.to_rfc3339()),
                doc.download_count as i64,
            ],
        )?;
        Ok(())
    }

    pub fn get(&self, id: &str) -> rusqlite::Result<Option<GeneratedDoc>> {
        let conn = self.db.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, template, format, filename, file_path, file_size_bytes,
                    related_tenants, generated_by, generated_at, expires_at, download_count
             FROM generated_docs WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row_to_doc(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn list_recent(&self, limit: u32) -> rusqlite::Result<Vec<GeneratedDoc>> {
        let conn = self.db.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, template, format, filename, file_path, file_size_bytes,
                    related_tenants, generated_by, generated_at, expires_at, download_count
             FROM generated_docs ORDER BY generated_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], row_to_doc)?;
        rows.collect()
    }

    pub fn increment_download(&self, id: &str) -> rusqlite::Result<()> {
        let conn = self.db.lock().unwrap();
        conn.execute(
            "UPDATE generated_docs SET download_count = download_count + 1 WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }
}

fn row_to_doc(row: &rusqlite::Row) -> rusqlite::Result<GeneratedDoc> {
    let related_str: Option<String> = row.get(6)?;
    let generated: String = row.get(8)?;
    let expires: Option<String> = row.get(9)?;
    Ok(GeneratedDoc {
        id: row.get(0)?,
        template: row.get(1)?,
        format: row.get(2)?,
        filename: row.get(3)?,
        file_path: row.get(4)?,
        file_size_bytes: row.get::<_, i64>(5)? as u64,
        related_tenants: related_str
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default(),
        generated_by: row.get(7)?,
        generated_at: parse_time(&generated),
        expires_at: expires.as_deref().map(parse_time),
        download_count: row.get::<_, i64>(10)? as u32,
    })
}

fn parse_time(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mec::db;

    fn sample(id: &str) -> GeneratedDoc {
        GeneratedDoc {
            id: id.into(),
            template: "tenant_access_guide".into(),
            format: "docx".into(),
            filename: format!("{}.docx", id),
            file_path: format!("/tmp/{}.docx", id),
            file_size_bytes: 4096,
            related_tenants: vec!["ygram-poc".into()],
            generated_by: Some("admin".into()),
            generated_at: Utc::now(),
            expires_at: None,
            download_count: 0,
        }
    }

    #[test]
    fn roundtrip() {
        let db = db::open_in_memory().unwrap();
        let store = DocsStore::new(db);
        store.insert(&sample("d1")).unwrap();
        let got = store.get("d1").unwrap().unwrap();
        assert_eq!(got.template, "tenant_access_guide");
        assert_eq!(got.related_tenants, vec!["ygram-poc".to_string()]);
    }

    #[test]
    fn increment() {
        let db = db::open_in_memory().unwrap();
        let store = DocsStore::new(db);
        store.insert(&sample("d2")).unwrap();
        store.increment_download("d2").unwrap();
        let got = store.get("d2").unwrap().unwrap();
        assert_eq!(got.download_count, 1);
    }
}
