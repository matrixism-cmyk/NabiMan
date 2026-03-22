use actix_web::{web, HttpResponse};
use crate::models::{ApiResponse, FirewallStatus, FirewallRule, FirewallAddRuleRequest, FirewallDeleteRuleRequest};

fn run_cmd(cmd: &str, args: &[&str]) -> Result<String, String> {
    let output = std::process::Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| format!("Failed to run {}: {}", cmd, e))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

enum FwBackend {
    Ufw,
    Firewalld,
    Iptables,
    None,
}

fn detect_backend() -> FwBackend {
    if std::process::Command::new("ufw").arg("version").output().map(|o| o.status.success()).unwrap_or(false) {
        FwBackend::Ufw
    } else if std::process::Command::new("firewall-cmd").arg("--version").output().map(|o| o.status.success()).unwrap_or(false) {
        FwBackend::Firewalld
    } else if std::process::Command::new("iptables").arg("-V").output().map(|o| o.status.success()).unwrap_or(false) {
        FwBackend::Iptables
    } else {
        FwBackend::None
    }
}

fn parse_ufw_rules(output: &str) -> Vec<FirewallRule> {
    let mut rules = Vec::new();
    let mut num = 1u32;
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("Status:") || trimmed.starts_with("To")
            || trimmed.starts_with("--") || trimmed.starts_with("Logging:")
            || trimmed.starts_with("Default:")
        {
            continue;
        }
        // Typical: 22/tcp ALLOW Anywhere
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 3 {
            let port_proto = parts[0];
            let action = parts[1].to_string();
            let source = parts[2..].join(" ");
            let (port, protocol) = if port_proto.contains('/') {
                let pp: Vec<&str> = port_proto.split('/').collect();
                (pp[0].to_string(), pp.get(1).unwrap_or(&"any").to_string())
            } else {
                (port_proto.to_string(), "any".to_string())
            };
            rules.push(FirewallRule {
                number: num,
                action,
                protocol,
                port,
                source,
                destination: "Anywhere".to_string(),
            });
            num += 1;
        }
    }
    rules
}

fn parse_iptables_rules(output: &str) -> Vec<FirewallRule> {
    let mut rules = Vec::new();
    let mut num = 1u32;
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("Chain") || trimmed.starts_with("target")
            || trimmed.starts_with("num")
        {
            continue;
        }
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 4 {
            let action = parts.first().unwrap_or(&"").to_string();
            let protocol = parts.get(1).unwrap_or(&"").to_string();
            let source = parts.get(3).unwrap_or(&"").to_string();
            let destination = parts.get(4).unwrap_or(&"").to_string();
            let port = if let Some(dpt) = parts.iter().find(|p| p.starts_with("dpt:")) {
                dpt.trim_start_matches("dpt:").to_string()
            } else {
                "*".to_string()
            };
            rules.push(FirewallRule { number: num, action, protocol, port, source, destination });
            num += 1;
        }
    }
    rules
}

async fn get_status() -> HttpResponse {
    match detect_backend() {
        FwBackend::Ufw => {
            let active = run_cmd("ufw", &["status"])
                .map(|o| o.contains("Status: active"))
                .unwrap_or(false);
            let rules_output = run_cmd("ufw", &["status"]).unwrap_or_default();
            let rules = parse_ufw_rules(&rules_output);
            HttpResponse::Ok().json(ApiResponse::ok(FirewallStatus {
                backend: "ufw".to_string(),
                active,
                rules,
            }))
        }
        FwBackend::Firewalld => {
            let active = run_cmd("firewall-cmd", &["--state"])
                .map(|o| o.trim() == "running")
                .unwrap_or(false);
            let ports_output = run_cmd("firewall-cmd", &["--list-all"]).unwrap_or_default();
            let mut rules = Vec::new();
            let mut num = 1u32;
            for line in ports_output.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("ports:") {
                    let ports_str = trimmed.trim_start_matches("ports:").trim();
                    for port_entry in ports_str.split_whitespace() {
                        let (port, proto) = if port_entry.contains('/') {
                            let pp: Vec<&str> = port_entry.split('/').collect();
                            (pp[0].to_string(), pp.get(1).unwrap_or(&"tcp").to_string())
                        } else {
                            (port_entry.to_string(), "tcp".to_string())
                        };
                        rules.push(FirewallRule {
                            number: num, action: "ALLOW".to_string(), protocol: proto,
                            port, source: "any".to_string(), destination: "any".to_string(),
                        });
                        num += 1;
                    }
                }
            }
            HttpResponse::Ok().json(ApiResponse::ok(FirewallStatus {
                backend: "firewalld".to_string(),
                active,
                rules,
            }))
        }
        FwBackend::Iptables => {
            let rules_output = run_cmd("iptables", &["-L", "INPUT", "-n", "--line-numbers"]).unwrap_or_default();
            let rules = parse_iptables_rules(&rules_output);
            HttpResponse::Ok().json(ApiResponse::ok(FirewallStatus {
                backend: "iptables".to_string(),
                active: true,
                rules,
            }))
        }
        FwBackend::None => {
            HttpResponse::Ok().json(ApiResponse::<FirewallStatus>::error("No firewall backend found"))
        }
    }
}

fn validate_port(port: &str) -> bool {
    if port.contains(':') {
        let parts: Vec<&str> = port.split(':').collect();
        parts.len() == 2 && parts.iter().all(|p| p.parse::<u16>().is_ok())
    } else {
        port.parse::<u16>().is_ok()
    }
}

fn validate_protocol(proto: &str) -> bool {
    matches!(proto, "tcp" | "udp" | "any")
}

async fn add_rule(body: web::Json<FirewallAddRuleRequest>) -> HttpResponse {
    if !validate_port(&body.port) {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid port"));
    }
    if !validate_protocol(&body.protocol) {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid protocol (tcp/udp/any)"));
    }

    let port_proto = if body.protocol == "any" {
        body.port.clone()
    } else {
        format!("{}/{}", body.port, body.protocol)
    };

    let result = match detect_backend() {
        FwBackend::Ufw => {
            let action = if body.action == "deny" { "deny" } else { "allow" };
            run_cmd("ufw", &[action, &port_proto])
        }
        FwBackend::Firewalld => {
            run_cmd("firewall-cmd", &["--add-port", &port_proto, "--permanent"])
                .and_then(|_| run_cmd("firewall-cmd", &["--reload"]))
        }
        FwBackend::Iptables => {
            let action_flag = if body.action == "deny" { "DROP" } else { "ACCEPT" };
            let proto = if body.protocol == "any" { "tcp" } else { &body.protocol };
            run_cmd("iptables", &["-A", "INPUT", "-p", proto, "--dport", &body.port, "-j", action_flag])
        }
        FwBackend::None => Err("No firewall backend found".to_string()),
    };

    match result {
        Ok(_) => HttpResponse::Ok().json(ApiResponse::ok(format!("Rule added: {} {}", body.action, port_proto))),
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e)),
    }
}

async fn delete_rule(body: web::Json<FirewallDeleteRuleRequest>) -> HttpResponse {
    let num_str = body.number.to_string();
    let result = match detect_backend() {
        FwBackend::Ufw => run_cmd("ufw", &["--force", "delete", &num_str]),
        FwBackend::Iptables => run_cmd("iptables", &["-D", "INPUT", &num_str]),
        _ => Err("Delete by number only supported for ufw/iptables".to_string()),
    };

    match result {
        Ok(_) => HttpResponse::Ok().json(ApiResponse::ok(format!("Rule {} deleted", body.number))),
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e)),
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/firewall")
            .route("/status", web::get().to(get_status))
            .route("/add", web::post().to(add_rule))
            .route("/delete", web::post().to(delete_rule)),
    );
}
