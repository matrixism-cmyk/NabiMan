//! A question put to Claude Code in headless mode, its answer streamed back
//! as it is written: one JSON object a line (`start`, `delta`, `tool`,
//! `note`, then `done` or `error`). The answer is kept with the conversation
//! even if the page is gone before it ends; "stop" ends it early.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot};

use super::chats::{self, Chat, Message};
use super::launch::Launch;
use super::Settings;
use crate::models::ApiResponse;

/// The longest an answer may take.
const LIMIT: Duration = Duration::from_secs(10 * 60);
const MOST_CHARS: usize = 8000;
const READ_TOOLS: &str = "Read,Glob,Grep,WebSearch,WebFetch";

/// How a spoken conversation differs from a typed one.
const VOICE_PROMPT: &str = "이 대화는 음성으로 이루어집니다. 사용자의 말은 음성 인식으로 받아 적은 것이라 잘못 들린 단어가 \
있을 수 있으니 문맥으로 이해하세요. 당신의 답변은 음성 합성으로 소리 내어 읽힙니다. 그러니 마크다운, 표, 코드 블록, 이모지, \
글머리 기호, URL을 쓰지 말고 말로 하듯 자연스러운 문장으로 답하세요. 길게 해 달라는 요청이 없으면 핵심만 서너 문장 이내로 \
짧게 답하세요. 사용자가 쓰는 언어로 답하세요.";

/// Answers being written, by conversation, with the way to stop each.
fn running() -> std::sync::MutexGuard<'static, HashMap<String, oneshot::Sender<()>>> {
    static MAP: OnceLock<Mutex<HashMap<String, oneshot::Sender<()>>>> = OnceLock::new();
    MAP.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
}

fn valid_session(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

/// The arguments for one question; the question itself goes on stdin.
pub(crate) fn args(s: &Settings, resume: Option<&str>) -> Vec<String> {
    let mut a: Vec<String> = ["-p", "--output-format", "stream-json", "--verbose", "--include-partial-messages", "--strict-mcp-config"]
        .iter().map(|x| x.to_string()).collect();
    if s.tools == "read" {
        a.extend(["--tools".into(), READ_TOOLS.into(), "--allowedTools".into(), READ_TOOLS.into()]);
    } else {
        a.extend(["--tools".into(), String::new()]);
    }
    if !s.model.is_empty() {
        a.extend(["--model".into(), s.model.clone()]);
    }
    let prompt = if s.extra_prompt.trim().is_empty() { VOICE_PROMPT.to_string() } else { format!("{VOICE_PROMPT}\n\n{}", s.extra_prompt.trim()) };
    a.extend(["--append-system-prompt".into(), prompt]);
    if let Some(id) = resume.filter(|id| valid_session(id)) {
        a.extend(["--resume".into(), id.to_string()]);
    }
    a
}

#[derive(Deserialize)]
pub(super) struct Ask {
    text: String,
}

pub(super) async fn ask(req: HttpRequest, id: web::Path<String>, body: web::Json<Ask>) -> HttpResponse {
    let who = match chats::owner(&req) { Ok(w) => w, Err(r) => return r };
    let Some(mut chat) = chats::load(&id, &who) else { return chats::not_found() };
    let text = body.text.trim().to_string();
    if text.is_empty() || text.chars().count() > MOST_CHARS {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error("Say something, up to 8000 characters"));
    }
    let settings = super::load();
    let launch = match Launch::resolve(&settings) {
        Ok(l) => l,
        Err(e) => return HttpResponse::ServiceUnavailable().json(ApiResponse::<()>::error(&e)),
    };
    let (stop_tx, stop_rx) = oneshot::channel();
    {
        let mut map = running();
        if map.contains_key(&chat.id) {
            return HttpResponse::Conflict().json(ApiResponse::<()>::error("Claude is still answering the last question"));
        }
        map.insert(chat.id.clone(), stop_tx);
    }
    if chat.title.is_empty() {
        chat.title = text.chars().take(40).collect();
    }
    chat.messages.push(Message { role: "user".into(), text: text.clone(), at: chats::now(), error: None });
    chat.updated = chats::now();
    let _ = chats::save(&chat);

    let (tx, rx) = mpsc::channel::<String>(512);
    actix_web::rt::spawn(async move {
        let _ = tx.send(line(json!({ "type": "start" }))).await;
        let outcome = run(&launch, &settings, &mut chat, &text, &tx, stop_rx).await;
        let (answer, error) = match outcome {
            Ok(answer) => (answer, None),
            Err((partial, e)) => (partial, Some(e)),
        };
        let _ = tx.send(line(match &error {
            None => json!({ "type": "done", "text": answer }),
            Some(e) => json!({ "type": "error", "message": e, "text": answer }),
        })).await;
        chat.messages.push(Message { role: "assistant".into(), text: answer, at: chats::now(), error });
        chat.updated = chats::now();
        let _ = chats::save(&chat);
        running().remove(&chat.id);
    });
    let body = futures::stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|l| (Ok::<_, actix_web::Error>(web::Bytes::from(l)), rx))
    });
    HttpResponse::Ok().content_type("application/x-ndjson")
        .insert_header(("Cache-Control", "no-store")).insert_header(("X-Accel-Buffering", "no"))
        .streaming(body)
}

pub(super) async fn stop(req: HttpRequest, id: web::Path<String>) -> HttpResponse {
    let who = match chats::owner(&req) { Ok(w) => w, Err(r) => return r };
    if chats::load(&id, &who).is_none() {
        return chats::not_found();
    }
    let stopped = running().remove(id.as_str()).map(|s| s.send(()).is_ok()).unwrap_or(false);
    HttpResponse::Ok().json(ApiResponse::ok(json!({ "stopped": stopped })))
}

fn line(v: Value) -> String {
    format!("{v}\n")
}

/// What one line of Claude's stream adds.
pub(crate) enum Event { Session(String), TextStart, Text(String), Tool(String), Result { ok: bool, text: String, session: String } }

pub(crate) fn parse(raw: &str) -> Option<Event> {
    let v: Value = serde_json::from_str(raw).ok()?;
    let s = |v: &Value| v.as_str().unwrap_or_default().to_string();
    match v["type"].as_str()? {
        "system" if v["subtype"] == "init" => Some(Event::Session(s(&v["session_id"]))),
        "stream_event" => {
            let e = &v["event"];
            match (e["type"].as_str()?, e["content_block"]["type"].as_str(), e["delta"]["type"].as_str()) {
                ("content_block_start", Some("text"), _) => Some(Event::TextStart),
                ("content_block_start", Some("tool_use"), _) => Some(Event::Tool(s(&e["content_block"]["name"]))),
                ("content_block_delta", _, Some("text_delta")) => Some(Event::Text(s(&e["delta"]["text"]))),
                _ => None,
            }
        }
        "result" => Some(Event::Result {
            ok: v["is_error"] != true && v["subtype"] == "success",
            text: if v["result"].is_string() { s(&v["result"]) } else { s(&v["subtype"]) },
            session: s(&v["session_id"]),
        }),
        _ => None,
    }
}

/// Runs Claude on `text`, passing the answer on as it comes. A conversation
/// whose Claude session is gone starts a fresh one, once, saying so.
async fn run(
    launch: &Launch, s: &Settings, chat: &mut Chat, text: &str,
    tx: &mpsc::Sender<String>, mut stop: oneshot::Receiver<()>,
) -> Result<String, (String, String)> {
    let mut resume = valid_session(&chat.session_id);
    loop {
        let mut child = launch.command(&args(s, resume.then_some(chat.session_id.as_str())))
            .spawn().map_err(|e| (String::new(), format!("Cannot start Claude: {e}")))?;
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes()).await;
        }
        let mut stderr = child.stderr.take();
        let errors = tokio::spawn(async move {
            let mut buf = String::new();
            if let Some(e) = stderr.as_mut() { let _ = e.take(16 * 1024).read_to_string(&mut buf).await; }
            buf
        });
        let mut lines = BufReader::new(child.stdout.take().expect("piped")).lines();
        let (mut answer, mut blocks, mut result) = (String::new(), 0, None);
        let deadline = tokio::time::sleep(LIMIT);
        tokio::pin!(deadline);
        loop {
            tokio::select! {
                next = lines.next_line() => match next {
                    Ok(Some(raw)) => match parse(&raw) {
                        Some(Event::Session(id)) if valid_session(&id) => chat.session_id = id,
                        Some(Event::TextStart) => {
                            if blocks > 0 && !answer.is_empty() {
                                answer.push_str("\n\n");
                                let _ = tx.send(line(json!({ "type": "delta", "text": "\n\n" }))).await;
                            }
                            blocks += 1;
                        }
                        Some(Event::Text(t)) => {
                            answer.push_str(&t);
                            let _ = tx.send(line(json!({ "type": "delta", "text": t }))).await;
                        }
                        Some(Event::Tool(name)) => { let _ = tx.send(line(json!({ "type": "tool", "name": name }))).await; }
                        Some(Event::Result { ok, text, session }) => result = Some((ok, text, session)),
                        _ => {}
                    },
                    _ => break,
                },
                _ = &mut stop => { let _ = child.kill().await; return Err((answer, "Stopped".into())); }
                _ = &mut deadline => { let _ = child.kill().await; return Err((answer, "Claude took longer than 10 minutes".into())); }
            }
        }
        let _ = child.wait().await;
        let stderr = errors.await.unwrap_or_default();
        match result {
            Some((true, final_text, session)) => {
                if valid_session(&session) { chat.session_id = session; }
                return Ok(if final_text.trim().is_empty() { answer } else { final_text });
            }
            failed if resume && answer.is_empty() => {
                // The session it would go on from is gone: start afresh, once.
                let _ = failed;
                resume = false;
                chat.session_id.clear();
                let _ = tx.send(line(json!({ "type": "note", "message": "fresh" }))).await;
            }
            failed => {
                let why = failed.map(|(_, t, _)| t).filter(|t| !t.is_empty())
                    .unwrap_or_else(|| stderr.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("Claude stopped without an answer").to_string());
                return Err((answer, why.chars().take(500).collect()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_question_runs_headless_without_tools_by_default() {
        let a = args(&Settings::default(), None);
        assert_eq!(&a[..2], ["-p", "--output-format"]);
        let tools = a.iter().position(|x| x == "--tools").unwrap();
        assert_eq!(a[tools + 1], "");
        assert!(a.contains(&"--strict-mcp-config".to_string()) && !a.contains(&"--resume".to_string()));
        let s = Settings { tools: "read".into(), model: "claude-opus-5-5".into(), ..Settings::default() };
        let a = args(&s, Some("28ca423b-84fa-4705-899f-9092055ab534"));
        assert!(a.windows(2).any(|w| w[0] == "--tools" && w[1] == READ_TOOLS));
        assert!(a.windows(2).any(|w| w[0] == "--model" && w[1] == "claude-opus-5-5"));
        assert!(a.windows(2).any(|w| w[0] == "--resume" && w[1] == "28ca423b-84fa-4705-899f-9092055ab534"));
        assert!(!args(&s, Some("--dangerously-skip-permissions")).contains(&"--resume".to_string()));
    }

    #[test]
    fn the_stream_is_read_as_claude_writes_it() {
        let init = r#"{"type":"system","subtype":"init","session_id":"28ca423b-84fa"}"#;
        assert!(matches!(parse(init), Some(Event::Session(id)) if id == "28ca423b-84fa"));
        let delta = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"햇빛이"}}}"#;
        assert!(matches!(parse(delta), Some(Event::Text(t)) if t == "햇빛이"));
        let thinking = r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"thinking_delta","thinking":"..."}}}"#;
        assert!(parse(thinking).is_none());
        let tool = r#"{"type":"stream_event","event":{"type":"content_block_start","content_block":{"type":"tool_use","name":"Read"}}}"#;
        assert!(matches!(parse(tool), Some(Event::Tool(n)) if n == "Read"));
        let done = r#"{"type":"result","subtype":"success","is_error":false,"result":"파랗게 보입니다.","session_id":"abc"}"#;
        assert!(matches!(parse(done), Some(Event::Result { ok: true, text, .. }) if text == "파랗게 보입니다."));
        let failed = r#"{"type":"result","subtype":"error_during_execution","is_error":true,"session_id":"abc"}"#;
        assert!(matches!(parse(failed), Some(Event::Result { ok: false, text, .. }) if text == "error_during_execution"));
    }
}
