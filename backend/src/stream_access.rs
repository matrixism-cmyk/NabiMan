//! Fresh account authorization for long-lived connections: every SSE
//! stream, a terminal, dictation. The account, its sign-in and its
//! permissions are checked again before each delivery and each input, so a
//! revoked sign-in or a lost permission ends the connection.
use crate::{auth, jwt_sessions::SessionStore, users::UserStore};
use actix_web::{web, HttpRequest};
use futures::{stream, Stream, StreamExt};
use std::{convert::Infallible, time::Duration};

pub(crate) fn guard<S>(
    inner: S,
    grant: ConnectionGrant,
) -> impl Stream<Item = Result<web::Bytes, Infallible>> + Send
where
    S: Stream<Item = Result<web::Bytes, Infallible>> + Send + 'static,
{
    let mut ticker = tokio::time::interval(Duration::from_secs(1));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    stream::unfold(
        (Box::pin(inner), grant, ticker),
        |(mut inner, grant, mut ticker)| async move {
            loop {
                if !grant.valid() {
                    return None;
                }
                tokio::select! {
                    _=ticker.tick()=>{},
                    value=inner.next()=>{
                        if !grant.valid() {return None;}
                        return value.map(|value|(value,(inner,grant,ticker)));
                    }
                }
            }
        },
    )
}

/// Checked before each account input and periodically while attached.
#[derive(Clone)]
pub(crate) struct ConnectionGrant {
    pub(crate) user_id: String,
    pub(crate) username: String,
    pub(crate) sid: String,
    pub(crate) sessions: SessionStore,
    pub(crate) users: UserStore,
    pub(crate) path: String,
}
impl ConnectionGrant {
    pub(crate) fn from_request(
        req: &HttpRequest,
        sessions: SessionStore,
        users: UserStore,
    ) -> Option<Self> {
        let claims = auth::verified_claims(req)?;
        Some(Self {
            user_id: auth::verified_user_id(req).filter(|id| !id.is_empty())?,
            username: claims.sub,
            sid: claims.sid,
            sessions,
            users,
            path: req.path().into(),
        })
    }
    pub(crate) fn valid(&self) -> bool {
        self.valid_for_method("GET")
    }
    pub(crate) fn valid_for_method(&self, method: &str) -> bool {
        if self.user_id.is_empty() {
            return false;
        }
        let session = self.sessions.lock().unwrap().get(&self.sid).cloned();
        let Some(session) =
            session.filter(|s| s.username == self.username && s.user_id == self.user_id)
        else {
            return false;
        };
        let Some(created) =
            chrono::NaiveDateTime::parse_from_str(&session.created_at, "%Y-%m-%d %H:%M:%S").ok()
        else {
            return false;
        };
        let age = chrono::Utc::now().signed_duration_since(created.and_utc());
        if age < chrono::Duration::zero() || age >= chrono::Duration::days(7) {
            return false;
        }
        crate::users::find_user_by_name(&self.users, &self.username).is_some_and(|u| {
            u.id == self.user_id
                && crate::mfa_policy::permitted(&u, session.mfa_verified, method, &self.path)
                && crate::scope_access::permission(&u, method, &self.path)
        })
    }
}
