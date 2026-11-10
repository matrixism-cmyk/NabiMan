//! The socket's requests that carry an `op` (see `claude_control_ops`),
//! sorted out beside the original two: opening a session here, and `list`.

use super::super::claude_control_ops::{self as ops, Caller, Ctx};
use super::{open, refuse, user_of, Answer, Ask};

pub(super) fn json(a: Answer) -> serde_json::Value {
    serde_json::to_value(a).unwrap_or_else(|_| serde_json::json!({"ok": false}))
}

/// One request: the operations of `claude_control_ops`, or the first two.
pub(super) fn answer(ask: Ask, uid: u32, pid: u32, ctx: &Ctx) -> serde_json::Value {
    let Some(op) = ask.op.clone() else {
        return json(open(ask, uid, pid, &ctx.store));
    };
    if op == "guide" {
        return ops::guide();
    }
    let Some(os_user) = user_of(uid) else {
        return json(refuse("this user cannot use NabiMan sessions"));
    };
    let caller = match Caller::find(&os_user, pid, &ctx.store) {
        Ok(c) => c,
        Err(e) => return json(refuse(e)),
    };
    let who = ask.who.as_deref().unwrap_or("");
    match op.as_str() {
        "status" => ops::status(who, ask.lines, &caller, ctx),
        "close" => ops::close(who, ask.force, &caller, ctx),
        "open" => match ask.server {
            Some(server) => super::super::claude_remote_start::open(
                super::super::claude_remote_start::RemoteOpen {
                    server,
                    dir: ask.dir,
                    title: ask.title,
                    resume: ask.resume,
                    skip_permissions: ask.skip_permissions,
                    prompt: ask.prompt,
                    wait: ask.wait,
                },
                &caller,
                ctx,
            ),
            None => json(open(ask, uid, pid, &ctx.store)),
        },
        other => json(refuse(format!("unknown op {other:?}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_operations_ride_on_the_same_strict_line() {
        let ask: Ask = serde_json::from_str(r#"{"op":"status","who":"x-review","lines":40}"#).unwrap();
        assert_eq!((ask.op.as_deref(), ask.who.as_deref(), ask.lines, ask.force), (Some("status"), Some("x-review"), Some(40), false));
        let ask: Ask = serde_json::from_str(r#"{"op":"close","who":"ab12cd","force":true}"#).unwrap();
        assert!(ask.force && ask.op.as_deref() == Some("close"));
        let ask: Ask = serde_json::from_str(r#"{"op":"open","server":"gpu-01","dir":"~/x","resume":false}"#).unwrap();
        assert_eq!(ask.server.as_deref(), Some("gpu-01"));
        assert!(!ask.resume && ask.skip_permissions && ask.wait);
        assert!(serde_json::from_str::<Ask>(r#"{"op":"close","who":"x","kill":true}"#).is_err());
        assert!(serde_json::from_str::<Ask>(r#"{"op":"status","lines":"many"}"#).is_err());
        assert_eq!(ops::guide()["ok"], true);
    }
}
