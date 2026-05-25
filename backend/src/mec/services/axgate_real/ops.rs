use super::client::{upstream, AxgateReal};
use super::parser::parse_nat_rules;
use super::session::AxgateSession;
use crate::mec::services::{ServiceError, ServiceResult};
use crate::models::mec::{
    AnyMarker, CreateNatRuleRequest, NatRule, NatSource, NatType, Protocol,
};
use chrono::Utc;
use std::time::Duration;

pub async fn add_nat_rule(
    r: &AxgateReal,
    req: &CreateNatRuleRequest,
) -> ServiceResult<NatRule> {
    let cfg = r.config();
    let mut session = AxgateSession::connect(
        &cfg.host,
        cfg.port,
        &cfg.username,
        &cfg.password,
        Duration::from_secs(cfg.timeout_secs),
    )
    .await?;

    let existing = parse_nat_rules(&session.fetch_running_config().await?);
    let next_id = next_id(&existing);
    let label = escape(&req.label);
    let mut commands = vec![
        format!("configure terminal"),
        format!(
            "ip nat policy from untrust to trust {}",
            next_id
        ),
        format!("dnat destination {}/32", req.public_ip),
        format!("translated {}", req.private_ip),
        format!("source {}", req.source),
    ];
    for port in &req.ports {
        commands.push(format!(
            "service-group {}_{}",
            protocol_prefix(&req.protocol),
            port
        ));
    }
    commands.push(format!("label \"{}\"", label));
    if req.enable_immediately {
        commands.push("enable".into());
    }
    commands.push("exit".into());
    if req.create_proxy_arp {
        commands.push("interface eth0-0".into());
        commands.push(format!("ip proxy-arp alias {}", req.public_ip));
        commands.push("exit".into());
    }
    commands.push("write memory".into());

    let refs: Vec<&str> = commands.iter().map(String::as_str).collect();
    session.run_commands(&refs).await?;

    Ok(NatRule {
        id: format!("nat-{}", next_id),
        from_zone: "untrust".into(),
        to_zone: "trust".into(),
        rule_type: NatType::Dnat,
        label: Some(req.label.clone()),
        source: if req.source == "any" {
            NatSource::Any(AnyMarker { any: true })
        } else {
            NatSource::Groups {
                groups: vec![req.source.clone()],
            }
        },
        destination: vec![format!("{}/32", req.public_ip)],
        service_groups: req
            .ports
            .iter()
            .map(|p| format!("{}_{}", protocol_prefix(&req.protocol), p))
            .collect(),
        translated_to: req.private_ip.clone(),
        enabled: req.enable_immediately,
        hits: 0,
        last_hit: None,
        created_at: Utc::now(),
    })
}

pub async fn delete_nat_rule(r: &AxgateReal, id: &str) -> ServiceResult<()> {
    let numeric = id
        .strip_prefix("nat-")
        .ok_or_else(|| ServiceError::InvalidInput(format!("invalid nat id: {}", id)))?;

    let cfg = r.config();
    let mut session = AxgateSession::connect(
        &cfg.host,
        cfg.port,
        &cfg.username,
        &cfg.password,
        Duration::from_secs(cfg.timeout_secs),
    )
    .await?;
    let commands = [
        "configure terminal".to_string(),
        format!("no ip nat policy from untrust to trust {}", numeric),
        "write memory".to_string(),
    ];
    let refs: Vec<&str> = commands.iter().map(String::as_str).collect();
    session.run_commands(&refs).await?;
    Ok(())
}

pub async fn toggle_nat_rule(
    r: &AxgateReal,
    id: &str,
    enabled: bool,
) -> ServiceResult<()> {
    let numeric = id
        .strip_prefix("nat-")
        .ok_or_else(|| ServiceError::InvalidInput(format!("invalid nat id: {}", id)))?;

    let cfg = r.config();
    let mut session = AxgateSession::connect(
        &cfg.host,
        cfg.port,
        &cfg.username,
        &cfg.password,
        Duration::from_secs(cfg.timeout_secs),
    )
    .await?;
    let action = if enabled { "enable" } else { "no enable" };
    let commands = [
        "configure terminal".to_string(),
        format!("ip nat policy from untrust to trust {}", numeric),
        action.to_string(),
        "exit".to_string(),
        "write memory".to_string(),
    ];
    let refs: Vec<&str> = commands.iter().map(String::as_str).collect();
    session.run_commands(&refs).await?;
    Ok(())
}

fn next_id(existing: &[NatRule]) -> u32 {
    existing
        .iter()
        .filter_map(|r| r.id.strip_prefix("nat-").and_then(|s| s.parse::<u32>().ok()))
        .max()
        .map(|m| m + 1)
        .unwrap_or(10)
}

fn protocol_prefix(p: &Protocol) -> &'static str {
    match p {
        Protocol::Tcp => "TCP",
        Protocol::Udp => "UDP",
        Protocol::Icmp => "ICMP",
    }
}

fn escape(s: &str) -> String {
    s.replace('"', "'")
}

// Suppresses unused import in non-feature builds.
#[allow(dead_code)]
fn _touch() {
    let _ = upstream("");
}
