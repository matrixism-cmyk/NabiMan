//! Fresh account authorization before SSE delivery, including idle connections.
use crate::terminal_tickets::ConnectionGrant;
use actix_web::web;
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
