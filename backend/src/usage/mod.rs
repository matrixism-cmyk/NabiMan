//! `GET /api/usage`: what this instance is used for, for whoever runs it and
//! bills for it — users, saved servers, terminal sessions open and the
//! session-hours of a period, data-directory storage, the licence. Super
//! administrators, or a collector with `Authorization: Bearer
//! <NABIMAN_USAGE_TOKEN>` (checked in `auth`, like the metrics token).
//! `docs/commercialization/hosting-model.md` describes every field.

pub(crate) mod ledger;
mod storage;

use crate::lock_safe::LockExt;
use crate::models::{AccountStatus, ApiResponse, UserRole};
use actix_web::{web, HttpResponse};
use chrono::{Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

pub(crate) const PATH: &str = "/api/usage";
const MAX_DAYS: i64 = 400;

/// Whether the request carries the usage collector's token.
pub(crate) fn token_matches(headers: &actix_web::http::header::HeaderMap) -> bool {
    crate::observability::metrics::bearer_setting_in(headers, &crate::install_config::values::env, "NABIMAN_USAGE_TOKEN")
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Ask {
    from: Option<String>,
    to: Option<String>,
}

#[derive(Serialize, Debug, PartialEq)]
pub(crate) struct Period {
    pub from: String,
    pub to: String,
    pub days: i64,
}

/// UTC days, both included; the last 30 days by default.
pub(crate) fn period(ask: &Ask, today: NaiveDate) -> Result<Period, String> {
    let day = |v: &Option<String>| -> Result<Option<NaiveDate>, String> {
        v.as_deref()
            .map(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| "Dates must be YYYY-MM-DD".to_string()))
            .transpose()
    };
    let to = day(&ask.to)?.unwrap_or(today);
    let from = day(&ask.from)?.unwrap_or(to - Duration::days(29));
    let days = (to - from).num_days() + 1;
    if days < 1 {
        return Err("`from` must not be after `to`".into());
    }
    if days > MAX_DAYS {
        return Err(format!("A period is at most {MAX_DAYS} days"));
    }
    Ok(Period { from: from.to_string(), to: to.to_string(), days })
}

#[derive(Serialize, Default, Debug, PartialEq)]
pub(crate) struct Users {
    pub total: usize,
    pub active: usize,
    pub pending: usize,
    pub suspended: usize,
    pub rejected: usize,
    pub admins: usize,
    pub operators: usize,
    pub viewers: usize,
    pub remote_members: usize,
}

pub(crate) fn users(all: &[crate::models::NabimanUser]) -> Users {
    let mut u = Users { total: all.len(), ..Default::default() };
    for user in all {
        match user.status {
            AccountStatus::Active => u.active += 1,
            AccountStatus::Pending => u.pending += 1,
            AccountStatus::Suspended => u.suspended += 1,
            AccountStatus::Rejected => u.rejected += 1,
        }
        if user.status == AccountStatus::Active {
            match user.role {
                UserRole::Admin => u.admins += 1,
                UserRole::Operator => u.operators += 1,
                UserRole::Viewer => u.viewers += 1,
                UserRole::Remote => u.remote_members += 1,
            }
        }
    }
    u
}

fn hours(seconds: i64) -> f64 {
    (seconds as f64 / 36.0).round() / 100.0
}

#[derive(Serialize)]
struct Licence {
    license_id: Option<String>,
    tier: String,
    plan: Option<String>,
    deployment: Option<String>,
    validation_state: String,
    expires_at: Option<String>,
    max_users: Option<u32>,
    max_servers: Option<u32>,
    plan_features: Vec<String>,
}

fn licence() -> Licence {
    let s = crate::license::current();
    let plan = s.limits.clone().unwrap_or_default();
    Licence {
        plan: s.limits.as_ref().map(|_| plan.plan.clone()),
        deployment: plan.deployment,
        max_users: plan.max_users,
        max_servers: if s.limits.is_some() { plan.max_servers } else { s.max_servers },
        plan_features: plan.features,
        license_id: s.license_id,
        tier: s.tier,
        validation_state: s.validation_state,
        expires_at: s.expires_at,
    }
}

async fn get_usage(
    users_store: web::Data<crate::users::UserStore>,
    sessions: web::Data<crate::terminal::PtyStore>,
    identity: web::Data<crate::runtime_identity::RuntimeIdentity>,
    ask: web::Query<Ask>,
) -> HttpResponse {
    let period = match period(&ask, Utc::now().date_naive()) {
        Ok(p) => p,
        Err(e) => return HttpResponse::BadRequest().json(ApiResponse::<()>::error(&e)),
    };
    let users = users(&users_store.lock_safe());
    let (open, attached) = {
        let map = sessions.lock_safe();
        (map.len(), map.values().filter(|s| s.ws_count > 0).count())
    };
    let (from, to) = (period.from.clone(), period.to.clone());
    let measured = web::block(move || {
        let servers = crate::remote_servers::load_servers().unwrap_or_default();
        let totals = ledger::with_db(|db| ledger::totals(db, &from, &to)).unwrap_or_default();
        (servers, totals, storage::measure(&nabiman_server::data_dir()), licence())
    })
    .await;
    let Ok((servers, totals, storage, licence)) = measured else {
        return HttpResponse::InternalServerError().json(ApiResponse::<()>::error("Usage could not be measured"));
    };
    let body = serde_json::json!({
        "generated_at": Utc::now().to_rfc3339(),
        "period": period,
        "instance": identity.get_ref(),
        "users": users,
        "servers": {
            "saved": servers.len(),
            "member_owned": servers.iter().filter(|s| s.member_owned()).count(),
            "peak": totals.peak_servers,
        },
        "terminal_sessions": {
            "open": open,
            "attached": attached,
            "peak_open": totals.peak_sessions,
            "session_hours": hours(totals.session_seconds),
            "attached_hours": hours(totals.attached_seconds),
        },
        "metering": {
            "sample_interval_secs": ledger::INTERVAL.as_secs(),
            "sampled_days": totals.sampled_days,
            "first_sampled_day": totals.first_day,
            "instance_hours": hours(totals.up_seconds),
            "peak_active_users": totals.peak_users,
        },
        "storage": storage,
        "licence": licence,
    });
    HttpResponse::Ok().insert_header(("Cache-Control", "no-store")).json(ApiResponse::ok(body))
}

/// Samples the stores into the ledger once a minute.
pub(crate) fn start_sampler(sessions: crate::terminal::PtyStore, users: crate::users::UserStore) {
    ledger::start(move || {
        let (open, attached) = {
            let map = sessions.lock_safe();
            (map.len() as u64, map.values().filter(|s| s.ws_count > 0).count() as u64)
        };
        ledger::Sample {
            seconds: 0,
            sessions: open,
            attached,
            users: crate::license::plan::seats(&users.lock_safe()) as u64,
            servers: crate::remote_servers::load_servers().map_or(0, |s| s.len() as u64),
        }
    });
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.route(PATH, web::get().to(get_usage));
}

#[cfg(test)]
mod tests;
