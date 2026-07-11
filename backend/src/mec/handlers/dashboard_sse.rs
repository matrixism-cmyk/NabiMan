//! Single multiplexed dashboard stream.
//!
//! One background poller refreshes the live snapshot on a fixed interval and
//! publishes it on a `watch` channel. Every connected wall subscribes to that
//! same channel, so N open dashboards cost ONE set of kube calls instead of
//! N×(interval) — the fan-out that plain per-client polling would incur.

use super::dashboard::build_live;
use crate::mec::services::ServiceBundle;
use crate::mec::MecState;
use actix_web::{web, HttpResponse};
use futures::stream::{self, Stream, StreamExt};
use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio_stream::wrappers::WatchStream;

type SseStream = Pin<Box<dyn Stream<Item = Result<web::Bytes, Infallible>> + Send>>;

const POLL_INTERVAL: Duration = Duration::from_secs(10);
const HEARTBEAT: Duration = Duration::from_secs(15);

/// Spawn the shared poll loop. Call once at startup, inside the async runtime.
pub fn start_poller(state: &web::Data<MecState>) {
    let services: Arc<ServiceBundle> = state.services.clone();
    let tx: watch::Sender<Option<crate::models::mec::LiveSnapshot>> = state.live_tx.clone();
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(POLL_INTERVAL);
        loop {
            ticker.tick().await;
            match build_live(&services).await {
                // Send even if there are no receivers yet — subscribers get the
                // latest value immediately on connect.
                Ok(snap) => {
                    let _ = tx.send(Some(snap));
                }
                // A transient upstream failure (kube 401, metrics-server down)
                // keeps the last good snapshot; the client shows it as stale.
                Err(e) => eprintln!("[mec] dashboard poll failed: {}", e),
            }
        }
    });
}

/// GET /dashboard/live/stream — snapshot-then-stream over Server-Sent Events.
pub async fn stream(state: web::Data<MecState>) -> HttpResponse {
    let rx = state.live_tx.subscribe();
    // WatchStream yields the current value first, then every subsequent change.
    let snapshots = WatchStream::new(rx).filter_map(|maybe| async move {
        maybe.map(|snap| {
            let body = serde_json::to_string(&snap).unwrap_or_else(|_| "{}".into());
            Ok::<web::Bytes, Infallible>(web::Bytes::from(sse_event("snapshot", &body)))
        })
    });
    let stream: SseStream = Box::pin(with_heartbeat(snapshots));

    HttpResponse::Ok()
        .content_type("text/event-stream")
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("Connection", "keep-alive"))
        .insert_header(("X-Accel-Buffering", "no"))
        .streaming(stream)
}

fn sse_event(name: &str, data: &str) -> String {
    format!("event: {}\ndata: {}\n\n", name, data)
}

fn with_heartbeat<S>(inner: S) -> impl Stream<Item = Result<web::Bytes, Infallible>> + Send
where
    S: Stream<Item = Result<web::Bytes, Infallible>> + Send + 'static,
{
    let hb = tokio_stream::wrappers::IntervalStream::new(tokio::time::interval(HEARTBEAT))
        .map(|_| Ok(web::Bytes::from_static(b": keepalive\n\n")));
    stream::select(inner, hb)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_event_shape() {
        let s = sse_event("snapshot", "{\"a\":1}");
        assert!(s.starts_with("event: snapshot\n"));
        assert!(s.contains("data: {\"a\":1}"));
        assert!(s.ends_with("\n\n"));
    }
}
