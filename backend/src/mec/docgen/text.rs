use super::super::db::docs_store::{DocsStore, GeneratedDoc};
use crate::models::mec::Tenant;
use chrono::Utc;
use std::path::PathBuf;
use std::sync::Arc;

pub struct DocGenerator {
    store: Arc<DocsStore>,
    output_dir: PathBuf,
}

impl DocGenerator {
    pub fn new(store: Arc<DocsStore>, output_dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&output_dir);
        Self { store, output_dir }
    }

    pub fn default_output_dir() -> PathBuf {
        std::env::var("NABIMAN_MEC_DOCS")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./data/mec_docs"))
    }

    pub fn generate_tenant_guide_text(&self, tenant: &Tenant) -> String {
        let alloc = &tenant.allocation;
        let quota = &tenant.quota;
        let gpu_label = alloc.gpu_label.as_deref().unwrap_or("none");
        let node = alloc.node.as_deref().unwrap_or("shared");
        format!(
            "{title}\n\
             \n\
             1. 기본 정보\n\
             - 기업명: {display}\n\
             - 테넌트 ID: {id}\n\
             - 과제명: {task}\n\
             - 네임스페이스: {ns}\n\
             - Rancher Project: {proj}\n\
             \n\
             2. 자원 할당\n\
             - 할당 노드: {node}\n\
             - GPU: {gpu}\n\
             - CPU requests/limits: {cpu_r} / {cpu_l}\n\
             - Memory requests/limits: {mem_r} / {mem_l}\n\
             - GPU slots: {gpu_slots}\n\
             - Pods: {pods}\n\
             \n\
             3. 접속 정보\n\
             - Rancher URL: https://rancher.jcia.mec.local\n\
             - 사용자: {user}\n\
             - 기본 비밀번호: (별도 전달)\n\
             \n\
             생성 시각: {ts}\n",
            title = "5G MEC 테스트베드 접속 가이드",
            display = tenant.display_name,
            id = tenant.id,
            task = tenant.task_name.as_deref().unwrap_or("-"),
            ns = tenant.namespace,
            proj = tenant.rancher_project_id.as_deref().unwrap_or("-"),
            node = node,
            gpu = gpu_label,
            cpu_r = quota.cpu_requests,
            cpu_l = quota.cpu_limits,
            mem_r = quota.memory_requests,
            mem_l = quota.memory_limits,
            gpu_slots = quota.gpu,
            pods = quota.pods,
            user = tenant.rancher_user_id.as_deref().unwrap_or("-"),
            ts = Utc::now().to_rfc3339(),
        )
    }

    pub fn generate_tenant_guide_text_file(
        &self,
        tenant: &Tenant,
        generated_by: &str,
    ) -> std::io::Result<GeneratedDoc> {
        let content = self.generate_tenant_guide_text(tenant);
        let id = uuid::Uuid::new_v4().to_string();
        let filename = format!("access_guide_{}_{}.txt", tenant.id, &id[..8]);
        let file_path = self.output_dir.join(&filename);
        std::fs::write(&file_path, &content)?;
        let size = content.len() as u64;

        let doc = GeneratedDoc {
            id,
            template: "tenant_access_guide".into(),
            format: "txt".into(),
            filename,
            file_path: file_path.to_string_lossy().into_owned(),
            file_size_bytes: size,
            related_tenants: vec![tenant.id.clone()],
            generated_by: Some(generated_by.into()),
            generated_at: Utc::now(),
            expires_at: None,
            download_count: 0,
        };
        let _ = self.store.insert(&doc);
        Ok(doc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mec::db;
    use crate::models::mec::{AllocationType, NodeAllocation, ResourceQuota};
    use crate::models::mec::TenantStatus;

    fn sample_tenant() -> Tenant {
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

    #[test]
    fn text_contains_core_fields() {
        let db = db::open_in_memory().unwrap();
        let store = Arc::new(DocsStore::new(db));
        let gen = DocGenerator::new(store, std::env::temp_dir());
        let text = gen.generate_tenant_guide_text(&sample_tenant());
        assert!(text.contains("ygram-poc"));
        assert!(text.contains("gh200"));
        assert!(text.contains("mec-wn04"));
    }

    #[test]
    fn file_created_and_recorded() {
        let db = db::open_in_memory().unwrap();
        let store = Arc::new(DocsStore::new(db.clone()));
        let tmp = tempdir();
        let gen = DocGenerator::new(store.clone(), tmp.clone());
        let doc = gen
            .generate_tenant_guide_text_file(&sample_tenant(), "admin")
            .unwrap();
        assert!(doc.file_size_bytes > 0);
        assert!(std::path::Path::new(&doc.file_path).exists());
        assert!(store.get(&doc.id).unwrap().is_some());
    }

    fn tempdir() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "mec-docs-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }
}
