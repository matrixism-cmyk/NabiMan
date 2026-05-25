use super::util::from_service_error;
use crate::mec::job_runner::JobEvent;
use crate::mec::services::ServiceError;
use crate::mec::MecState;
use actix_web::{web, HttpResponse};
use futures::stream::{self, Stream, StreamExt};
use std::convert::Infallible;
use std::pin::Pin;
use std::time::Duration;
use tokio_stream::wrappers::BroadcastStream;

type SseStream = Pin<Box<dyn Stream<Item = Result<web::Bytes, Infallible>> + Send>>;

pub async fn stream(path: web::Path<String>, state: web::Data<MecState>) -> HttpResponse {
    let job_id = path.into_inner();

    let existing = state
        .job_store
        .get(&job_id)
        .unwrap_or(None);
    if existing.is_none() {
        return from_service_error(ServiceError::NotFound(format!("job {}", job_id)));
    }

    let rx = state.jobs.subscribe(&job_id);
    let initial = serde_json::json!({
        "event": "snapshot",
        "job_id": job_id,
        "status": existing.as_ref().map(|j| j.status.clone()),
    });
    let initial_bytes = sse_event("snapshot", &initial.to_string());

    let stream: SseStream = if let Some(rx) = rx {
        let live = BroadcastStream::new(rx).filter_map(|r| async move {
            match r {
                Ok(ev) => Some(Ok::<web::Bytes, Infallible>(format_event(ev))),
                Err(_) => None,
            }
        });
        let combined = stream::once(async move { Ok::<web::Bytes, Infallible>(web::Bytes::from(initial_bytes)) }).chain(live);
        Box::pin(with_heartbeat(combined))
    } else {
        // Terminal job: just emit snapshot then close.
        let once = stream::once(async move { Ok::<web::Bytes, Infallible>(web::Bytes::from(initial_bytes)) });
        Box::pin(once)
    };

    HttpResponse::Ok()
        .content_type("text/event-stream")
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("Connection", "keep-alive"))
        .insert_header(("X-Accel-Buffering", "no"))
        .streaming(stream)
}

fn format_event(e: JobEvent) -> web::Bytes {
    let name = match &e {
        JobEvent::Started { .. } => "started",
        JobEvent::StepStarted { .. } => "step_started",
        JobEvent::StepCompleted { .. } => "step_completed",
        JobEvent::StepFailed { .. } => "step_failed",
        JobEvent::Completed { .. } => "completed",
        JobEvent::Failed { .. } => "failed",
    };
    let body = serde_json::to_string(&e).unwrap_or_else(|_| "{}".into());
    web::Bytes::from(sse_event(name, &body))
}

fn sse_event(name: &str, data: &str) -> String {
    format!("event: {}\ndata: {}\n\n", name, data)
}

fn heartbeat_bytes() -> web::Bytes {
    web::Bytes::from_static(b": keepalive\n\n")
}

fn with_heartbeat<S>(inner: S) -> impl Stream<Item = Result<web::Bytes, Infallible>> + Send
where
    S: Stream<Item = Result<web::Bytes, Infallible>> + Send + 'static,
{
    let hb = tokio_stream::wrappers::IntervalStream::new(tokio::time::interval(Duration::from_secs(15)))
        .map(|_| Ok(heartbeat_bytes()));
    stream::select(inner, hb)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sse_format() {
        let s = sse_event("x", "{\"a\":1}");
        assert!(s.starts_with("event: x\n"));
        assert!(s.contains("data: {\"a\":1}"));
        assert!(s.ends_with("\n\n"));
    }
}
