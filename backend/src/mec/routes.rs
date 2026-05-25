use super::handlers::{
    audit, dashboard, discover, docs, firewall, gpu, health, jobs, jobs_sse, members, network,
    nodes, preflight, settings, storage, tenants, users,
};
use actix_web::web;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/mec/v1")
            .service(
                web::scope("/dashboard")
                    .route("/summary", web::get().to(dashboard::summary))
                    .route("/activity", web::get().to(dashboard::activity)),
            )
            .service(tenant_scope())
            .service(node_scope())
            .service(gpu_scope())
            .service(network_scope())
            .service(firewall_scope())
            .service(storage_scope())
            .service(audit_scope())
            .service(job_scope())
            .service(doc_scope())
            .service(user_scope())
            .route("/health/summary", web::get().to(health::summary))
            .route("/settings/snapshot", web::get().to(settings::snapshot)),
    );
}

fn user_scope() -> actix_web::Scope {
    web::scope("/users")
        .route("", web::get().to(users::list))
        .route("", web::post().to(users::create))
        .route("/{id}", web::get().to(users::get))
        .route("/{id}", web::delete().to(users::delete))
        .route("/{id}/reset-password", web::post().to(users::reset_password))
}

fn tenant_scope() -> actix_web::Scope {
    web::scope("/tenants")
        .route("", web::get().to(tenants::list))
        .route("", web::post().to(tenants::create))
        .route("/discover", web::get().to(discover::discover))
        .route("/{id}", web::get().to(tenants::get))
        .route("/{id}", web::delete().to(tenants::delete))
        .route("/{id}/import", web::post().to(discover::import_tenant))
        .route("/{id}/delete-preflight", web::get().to(preflight::tenant_delete))
        .route("/{id}/quota-usage", web::get().to(tenants::quota_usage))
        .route("/{id}/resources", web::get().to(tenants::resources))
        .route("/{id}/guide.txt", web::get().to(tenants::guide_download))
        .route("/{id}/guide.docx", web::get().to(tenants::guide_download_docx))
        .route("/{id}/starter-kit", web::post().to(tenants::deploy_starter_kit))
        .route("/{id}/starter-kit", web::delete().to(tenants::delete_starter_kit))
        .route("/{id}/members", web::get().to(members::list))
        .route("/{id}/members", web::post().to(members::add))
        .route("/{id}/members/{binding_id}", web::delete().to(members::remove))
}

fn node_scope() -> actix_web::Scope {
    web::scope("/nodes")
        .route("", web::get().to(nodes::list))
        .route("/{name}", web::get().to(nodes::get))
        .route("/{name}/pods", web::get().to(nodes::pods))
        .route("/{name}/labels", web::patch().to(nodes::patch_labels))
        .route("/{name}/taints", web::patch().to(nodes::patch_taints))
        .route("/{name}/cordon", web::post().to(nodes::cordon))
        .route("/{name}/uncordon", web::post().to(nodes::uncordon))
        .route("/{name}/gpu-mode", web::post().to(nodes::gpu_mode))
}

fn gpu_scope() -> actix_web::Scope {
    web::scope("/gpu")
        .route("/overview", web::get().to(gpu::overview))
        .route("/allocations", web::get().to(gpu::allocations))
        .route("/usage", web::get().to(gpu::usage))
}

fn network_scope() -> actix_web::Scope {
    web::scope("/network")
        .route("/lb-pools", web::get().to(network::lb_pools))
        .route("/services", web::get().to(network::services))
        .route("/ingresses", web::get().to(network::ingresses))
        .route("/ingresses", web::post().to(network::create_ingress))
        .route(
            "/ingresses/{namespace}/{name}",
            web::delete().to(network::delete_ingress),
        )
}

fn firewall_scope() -> actix_web::Scope {
    web::scope("/firewall")
        .route("/public-ips", web::get().to(firewall::public_ips))
        .route("/nat-rules", web::get().to(firewall::list_nat))
        .route("/nat-rules", web::post().to(firewall::add_nat))
        .route("/nat-rules/{id}", web::delete().to(firewall::delete_nat))
        .route("/nat-rules/{id}", web::patch().to(firewall::patch_nat))
        .route("/security-policies", web::get().to(firewall::security_policies))
        .route("/sync", web::post().to(firewall::sync))
}

fn storage_scope() -> actix_web::Scope {
    web::scope("/storage")
        .route("/overview", web::get().to(storage::overview))
        .route("/pvcs", web::get().to(storage::pvcs))
        .route("/harbor/projects", web::get().to(storage::harbor_projects))
        .route("/harbor/projects", web::post().to(storage::create_harbor_project))
        .route(
            "/harbor/projects/{project}/images",
            web::get().to(storage::harbor_repositories),
        )
}

fn audit_scope() -> actix_web::Scope {
    web::scope("/audit")
        .route("/logs", web::get().to(audit::list))
        .route("/logs/{id}", web::get().to(audit::get))
}

fn job_scope() -> actix_web::Scope {
    web::scope("/jobs")
        .route("", web::get().to(jobs::list))
        .route("/{id}", web::get().to(jobs::get))
        .route("/{id}/stream", web::get().to(jobs_sse::stream))
}

fn doc_scope() -> actix_web::Scope {
    web::scope("/docs")
        .route("/templates", web::get().to(docs::templates))
        .route("/generated", web::get().to(docs::list_generated))
        .route("/generate", web::post().to(docs::generate))
        .route("/downloads/{id}", web::get().to(docs::download))
}
