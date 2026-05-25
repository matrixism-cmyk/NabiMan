use super::super::db::docs_store::{DocsStore, GeneratedDoc};
use crate::models::mec::Tenant;
use chrono::Utc;
use docx_rs::{Docx, Paragraph, Run};
use std::path::PathBuf;
use std::sync::Arc;

pub struct DocxGenerator {
    store: Arc<DocsStore>,
    output_dir: PathBuf,
}

impl DocxGenerator {
    pub fn new(store: Arc<DocsStore>, output_dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&output_dir);
        Self { store, output_dir }
    }

    pub fn build_tenant_guide(&self, tenant: &Tenant) -> Docx {
        let alloc = &tenant.allocation;
        let quota = &tenant.quota;
        let title = "5G MEC 테스트베드 접속 가이드";

        let mut doc = Docx::new()
            .add_paragraph(heading(title))
            .add_paragraph(blank())
            .add_paragraph(section_heading("1. 기본 정보"));

        doc = add_kv(doc, "기업명", &tenant.display_name);
        doc = add_kv(doc, "테넌트 ID", &tenant.id);
        doc = add_kv(doc, "과제명", tenant.task_name.as_deref().unwrap_or("-"));
        doc = add_kv(doc, "네임스페이스", &tenant.namespace);
        doc = add_kv(
            doc,
            "Rancher Project",
            tenant.rancher_project_id.as_deref().unwrap_or("-"),
        );

        doc = doc
            .add_paragraph(blank())
            .add_paragraph(section_heading("2. 자원 할당"));
        doc = add_kv(doc, "할당 노드", alloc.node.as_deref().unwrap_or("shared"));
        doc = add_kv(doc, "GPU", alloc.gpu_label.as_deref().unwrap_or("none"));
        doc = add_kv(
            doc,
            "CPU requests/limits",
            &format!("{} / {}", quota.cpu_requests, quota.cpu_limits),
        );
        doc = add_kv(
            doc,
            "Memory requests/limits",
            &format!("{} / {}", quota.memory_requests, quota.memory_limits),
        );
        doc = add_kv(doc, "GPU slots", &quota.gpu.to_string());
        doc = add_kv(doc, "Pods", &quota.pods.to_string());
        doc = add_kv(doc, "Storage", &quota.storage);

        doc = doc
            .add_paragraph(blank())
            .add_paragraph(section_heading("3. 접속 정보"));
        doc = add_kv(doc, "Rancher URL", "https://rancher.jcia.mec.local");
        doc = add_kv(
            doc,
            "사용자",
            tenant.rancher_user_id.as_deref().unwrap_or("-"),
        );
        doc = add_kv(doc, "기본 비밀번호", "(별도 전달)");

        doc = doc
            .add_paragraph(blank())
            .add_paragraph(footer_text(&Utc::now().to_rfc3339()));

        doc
    }

    pub fn generate_tenant_guide(
        &self,
        tenant: &Tenant,
        generated_by: &str,
    ) -> std::io::Result<GeneratedDoc> {
        let doc = self.build_tenant_guide(tenant);
        let id = uuid::Uuid::new_v4().to_string();
        let filename = format!("access_guide_{}_{}.docx", tenant.id, &id[..8]);
        let file_path = self.output_dir.join(&filename);
        let file = std::fs::File::create(&file_path)?;
        doc.build()
            .pack(file)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        let metadata = std::fs::metadata(&file_path)?;
        let record = GeneratedDoc {
            id,
            template: "tenant_access_guide".into(),
            format: "docx".into(),
            filename,
            file_path: file_path.to_string_lossy().into_owned(),
            file_size_bytes: metadata.len(),
            related_tenants: vec![tenant.id.clone()],
            generated_by: Some(generated_by.into()),
            generated_at: Utc::now(),
            expires_at: None,
            download_count: 0,
        };
        let _ = self.store.insert(&record);
        Ok(record)
    }
}

fn heading(text: &str) -> Paragraph {
    Paragraph::new().add_run(Run::new().add_text(text).size(32).bold())
}

fn section_heading(text: &str) -> Paragraph {
    Paragraph::new().add_run(Run::new().add_text(text).size(24).bold())
}

fn add_kv(doc: Docx, key: &str, value: &str) -> Docx {
    doc.add_paragraph(
        Paragraph::new()
            .add_run(Run::new().add_text(format!("{}: ", key)).bold())
            .add_run(Run::new().add_text(value)),
    )
}

fn blank() -> Paragraph {
    Paragraph::new().add_run(Run::new().add_text(""))
}

fn footer_text(ts: &str) -> Paragraph {
    Paragraph::new().add_run(
        Run::new()
            .add_text(format!("생성 시각: {}", ts))
            .size(18)
            .italic(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mec::db;
    use crate::models::mec::{
        AllocationType, NodeAllocation, ResourceQuota, TenantStatus,
    };

    fn sample() -> Tenant {
        Tenant {
            id: "ygram-poc".into(),
            display_name: "와이그램".into(),
            task_name: Some("AI Toy".into()),
            contact_email: None,
            namespace: "ygram-poc".into(),
            rancher_project_id: Some("p-xxxx".into()),
            rancher_user_id: Some("u-yyyy".into()),
            allocation: NodeAllocation {
                alloc_type: AllocationType::Dedicated,
                node: Some("mec-wn04".into()),
                gpu_label: Some("gh200".into()),
                tier: Some("high".into()),
            },
            quota: ResourceQuota::standard(),
            starter_kit: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            status: TenantStatus::Active,
        }
    }

    fn tempdir() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "mec-docx-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn creates_docx_file() {
        let db = db::open_in_memory().unwrap();
        let store = Arc::new(DocsStore::new(db));
        let gen = DocxGenerator::new(store.clone(), tempdir());
        let doc = gen.generate_tenant_guide(&sample(), "admin").unwrap();
        assert!(doc.file_size_bytes > 0);
        let path = std::path::Path::new(&doc.file_path);
        assert!(path.exists());
        // docx is a zip; first bytes are "PK".
        let bytes = std::fs::read(path).unwrap();
        assert_eq!(&bytes[..2], b"PK");
        assert!(store.get(&doc.id).unwrap().is_some());
    }
}
