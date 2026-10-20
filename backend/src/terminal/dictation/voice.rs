//! Reaching the speech service: its health over HTTP, and its live
//! transcription over a WebSocket, both signed in with the API key.

use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::WebSocketStream;

use super::Settings;
use crate::terminal::sftp::ftp::tls::Tls;

const CONNECT: Duration = Duration::from_secs(10);

/// Scheme, host and port of the service's URL.
fn target(s: &Settings) -> Result<(bool, String, u16), String> {
    let url = reqwest::Url::parse(s.url()).map_err(|_| "The dictation service address is not valid")?;
    let tls = url.scheme() == "https";
    let host = url.host_str().ok_or("The dictation service address has no host")?.to_string();
    let port = url.port_or_known_default().unwrap_or(if tls { 443 } else { 80 });
    Ok((tls, host, port))
}

/// `GET /api/health` with the key: whether the service is up and takes it.
pub(super) async fn health(s: &Settings, key: &str) -> Result<serde_json::Value, String> {
    let (_, host, port) = target(s)?;
    let mut client = reqwest::Client::builder().timeout(Duration::from_secs(15));
    if !s.address.is_empty() {
        let addr = tokio::net::lookup_host(&s.address).await.ok().and_then(|mut a| a.next())
            .ok_or("The connect-to address does not resolve")?;
        client = client.resolve(&host, std::net::SocketAddr::new(addr.ip(), if addr.port() == 0 { port } else { addr.port() }));
    }
    let client = client.build().map_err(|e| e.to_string())?;
    let res = client.get(format!("{}/api/health", s.url())).bearer_auth(key).send().await
        .map_err(|e| format!("Cannot reach the dictation service: {e}"))?;
    match res.status().as_u16() {
        200 => res.json().await.map_err(|e| format!("Unexpected answer: {e}")),
        401 | 403 => Err("The dictation service refused the API key".into()),
        code => Err(format!("The dictation service answered HTTP {code}")),
    }
}

/// An open live transcription, over TLS or (for a local service) not.
pub(super) enum Live {
    Plain(WebSocketStream<TcpStream>),
    Tls(Box<WebSocketStream<tokio_rustls::client::TlsStream<TcpStream>>>),
}

pub(super) async fn connect(s: &Settings, key: &str) -> Result<Live, String> {
    let (tls, host, port) = target(s)?;
    let dial = if s.address.is_empty() { format!("{host}:{port}") } else { s.address.clone() };
    let tcp = match tokio::time::timeout(CONNECT, TcpStream::connect(&dial)).await {
        Ok(Ok(tcp)) => tcp,
        Ok(Err(e)) => return Err(format!("Cannot reach the dictation service ({dial}): {e}")),
        Err(_) => return Err(format!("The dictation service ({dial}) did not answer")),
    };
    let _ = tcp.set_nodelay(true);
    let scheme = if tls { "wss" } else { "ws" };
    let mut request = format!("{scheme}://{host}:{port}/api/stt/stream").into_client_request().map_err(|e| e.to_string())?;
    let auth = HeaderValue::from_str(&format!("Bearer {key}")).map_err(|_| "The API key is not valid")?;
    request.headers_mut().insert("Authorization", auth);
    if !tls {
        return handshake(request, tcp).await.map(Live::Plain);
    }
    let settings = Arc::new(Tls::new(&host, "")?);
    let connector = tokio_rustls::TlsConnector::from(settings.config.clone());
    let stream = match tokio::time::timeout(CONNECT, connector.connect(settings.name.clone(), tcp)).await {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => return Err(settings.explain(&e)),
        Err(_) => return Err("TLS with the dictation service timed out".into()),
    };
    handshake(request, stream).await.map(|ws| Live::Tls(Box::new(ws)))
}

async fn handshake<S: AsyncRead + AsyncWrite + Unpin>(
    request: tokio_tungstenite::tungstenite::handshake::client::Request,
    stream: S,
) -> Result<WebSocketStream<S>, String> {
    use tokio_tungstenite::tungstenite::Error;
    match tokio::time::timeout(CONNECT, tokio_tungstenite::client_async(request, stream)).await {
        Ok(Ok((ws, _))) => Ok(ws),
        Ok(Err(Error::Http(res))) if matches!(res.status().as_u16(), 401 | 403) => {
            Err("The dictation service refused the API key".into())
        }
        Ok(Err(Error::Http(res))) => Err(format!("The dictation service answered HTTP {}", res.status())),
        Ok(Err(e)) => Err(format!("Cannot open live dictation: {e}")),
        Err(_) => Err("The dictation service did not open live dictation".into()),
    }
}
