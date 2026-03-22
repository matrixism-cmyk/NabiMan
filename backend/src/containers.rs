use actix_web::{web, HttpResponse};
use crate::models::{ApiResponse, Container, ContainerImage, ContainerActionRequest};

fn run_docker(args: &[&str]) -> Result<String, String> {
    let output = std::process::Command::new("docker")
        .args(args)
        .output()
        .map_err(|e| format!("Failed to run docker: {}", e))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

fn check_docker() -> bool {
    std::process::Command::new("docker")
        .arg("info")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

async fn list_containers() -> HttpResponse {
    if !check_docker() {
        return HttpResponse::Ok().json(ApiResponse::<Vec<Container>>::error("Docker is not available"));
    }

    match run_docker(&[
        "ps", "-a",
        "--format", "{{.ID}}\\t{{.Names}}\\t{{.Image}}\\t{{.Status}}\\t{{.State}}\\t{{.Ports}}\\t{{.CreatedAt}}",
    ]) {
        Ok(output) => {
            let containers: Vec<Container> = output
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|line| {
                    let parts: Vec<&str> = line.split('\t').collect();
                    Container {
                        id: parts.first().unwrap_or(&"").to_string(),
                        name: parts.get(1).unwrap_or(&"").to_string(),
                        image: parts.get(2).unwrap_or(&"").to_string(),
                        status: parts.get(3).unwrap_or(&"").to_string(),
                        state: parts.get(4).unwrap_or(&"").to_string(),
                        ports: parts.get(5).unwrap_or(&"").to_string(),
                        created: parts.get(6).unwrap_or(&"").to_string(),
                    }
                })
                .collect();
            HttpResponse::Ok().json(ApiResponse::ok(containers))
        }
        Err(e) => HttpResponse::Ok().json(ApiResponse::<Vec<Container>>::error(&e)),
    }
}

async fn list_images() -> HttpResponse {
    if !check_docker() {
        return HttpResponse::Ok().json(ApiResponse::<Vec<ContainerImage>>::error("Docker is not available"));
    }

    match run_docker(&[
        "images", "--format", "{{.ID}}\\t{{.Repository}}\\t{{.Tag}}\\t{{.Size}}\\t{{.CreatedAt}}",
    ]) {
        Ok(output) => {
            let images: Vec<ContainerImage> = output
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|line| {
                    let parts: Vec<&str> = line.split('\t').collect();
                    ContainerImage {
                        id: parts.first().unwrap_or(&"").to_string(),
                        repository: parts.get(1).unwrap_or(&"").to_string(),
                        tag: parts.get(2).unwrap_or(&"").to_string(),
                        size: parts.get(3).unwrap_or(&"").to_string(),
                        created: parts.get(4).unwrap_or(&"").to_string(),
                    }
                })
                .collect();
            HttpResponse::Ok().json(ApiResponse::ok(images))
        }
        Err(e) => HttpResponse::Ok().json(ApiResponse::<Vec<ContainerImage>>::error(&e)),
    }
}

async fn start_container(body: web::Json<ContainerActionRequest>) -> HttpResponse {
    let id = &body.id;
    if id.contains(|c: char| !c.is_alphanumeric() && c != '_' && c != '-' && c != '.') {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid container ID"));
    }
    match run_docker(&["start", id]) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse::ok(format!("Container {} started", id))),
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e)),
    }
}

async fn stop_container(body: web::Json<ContainerActionRequest>) -> HttpResponse {
    let id = &body.id;
    if id.contains(|c: char| !c.is_alphanumeric() && c != '_' && c != '-' && c != '.') {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid container ID"));
    }
    match run_docker(&["stop", id]) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse::ok(format!("Container {} stopped", id))),
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e)),
    }
}

async fn restart_container(body: web::Json<ContainerActionRequest>) -> HttpResponse {
    let id = &body.id;
    if id.contains(|c: char| !c.is_alphanumeric() && c != '_' && c != '-' && c != '.') {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid container ID"));
    }
    match run_docker(&["restart", id]) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse::ok(format!("Container {} restarted", id))),
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e)),
    }
}

async fn remove_container(body: web::Json<ContainerActionRequest>) -> HttpResponse {
    let id = &body.id;
    if id.contains(|c: char| !c.is_alphanumeric() && c != '_' && c != '-' && c != '.') {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid container ID"));
    }
    match run_docker(&["rm", "-f", id]) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse::ok(format!("Container {} removed", id))),
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e)),
    }
}

async fn container_logs(body: web::Json<ContainerActionRequest>) -> HttpResponse {
    let id = &body.id;
    if id.contains(|c: char| !c.is_alphanumeric() && c != '_' && c != '-' && c != '.') {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid container ID"));
    }
    match run_docker(&["logs", "--tail", "200", id]) {
        Ok(logs) => HttpResponse::Ok().json(ApiResponse::ok(logs)),
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e)),
    }
}

async fn remove_image(body: web::Json<ContainerActionRequest>) -> HttpResponse {
    let id = &body.id;
    if id.contains(|c: char| !c.is_alphanumeric() && c != '_' && c != '-' && c != '.' && c != ':' && c != '/') {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid image ID"));
    }
    match run_docker(&["rmi", id]) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse::ok(format!("Image {} removed", id))),
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e)),
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/containers")
            .route("", web::get().to(list_containers))
            .route("/images", web::get().to(list_images))
            .route("/start", web::post().to(start_container))
            .route("/stop", web::post().to(stop_container))
            .route("/restart", web::post().to(restart_container))
            .route("/remove", web::post().to(remove_container))
            .route("/logs", web::post().to(container_logs))
            .route("/images/remove", web::post().to(remove_image)),
    );
}
