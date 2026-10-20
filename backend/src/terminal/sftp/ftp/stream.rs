//! A download or upload as a byte stream. Each one has a control connection
//! to itself for as long as it runs, so browsing goes on beside it; when it
//! ends cleanly the server's closing reply is read and the connection goes
//! back to be reused.

use std::future::Future;
use std::io::{Error, ErrorKind};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{ready, Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use super::control::{Control, Io};
use super::FtpConn;

/// How long an upload waits for the server to close the data connection.
const DRAIN: std::time::Duration = std::time::Duration::from_secs(30);

type Finishing = Pin<Box<dyn Future<Output = std::io::Result<()>> + Send>>;

/// Reads the reply that ends a transfer and, if it was a success, returns the
/// connection for reuse. An upload's data connection is held open until then:
/// closing a socket with unread bytes in it (a TLS session ticket, say) resets
/// it, and a reset can throw away the end of the file before the server has
/// read it.
fn finishing(ctl: Option<Control>, conn: Arc<FtpConn>, hold: Option<Io>) -> Finishing {
    Box::pin(async move {
        let Some(mut ctl) = ctl else { return Ok(()) };
        if let Some(mut data) = hold {
            // The server closes its end once the file is stored. Reading up
            // to there also takes in the TLS session tickets it sent, which
            // the next data connection needs to resume the session.
            let mut sink = tokio::io::sink();
            let _ = tokio::time::timeout(DRAIN, tokio::io::copy(&mut data, &mut sink)).await;
        }
        let reply = ctl.finish().await;
        match reply {
            Ok(_) => {
                conn.give_back(ctl);
                Ok(())
            }
            Err(e) => Err(Error::other(format!("The server did not confirm the transfer: {e}"))),
        }
    })
}

pub(crate) struct Download {
    data: Option<Io>,
    ctl: Option<Control>,
    conn: Arc<FtpConn>,
    done: Option<Finishing>,
}

impl Download {
    pub fn new(data: Io, ctl: Control, conn: Arc<FtpConn>) -> Self {
        Download { data: Some(data), ctl: Some(ctl), conn, done: None }
    }
}

impl AsyncRead for Download {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        if let Some(data) = this.data.as_mut() {
            let before = buf.filled().len();
            match ready!(Pin::new(data).poll_read(cx, buf)) {
                Ok(()) if buf.filled().len() > before => return Poll::Ready(Ok(())),
                Ok(()) => {}
                // Servers often close a TLS data connection without saying
                // goodbye; the closing reply says whether everything came.
                Err(e) if e.kind() == ErrorKind::UnexpectedEof => {}
                Err(e) => return Poll::Ready(Err(e)),
            }
            this.data = None;
            this.done = Some(finishing(this.ctl.take(), this.conn.clone(), None));
        }
        if let Some(done) = this.done.as_mut() {
            let result = ready!(done.as_mut().poll(cx));
            this.done = None;
            result?;
        }
        Poll::Ready(Ok(()))
    }
}

pub(crate) struct Upload {
    data: Option<Io>,
    ctl: Option<Control>,
    conn: Arc<FtpConn>,
    done: Option<Finishing>,
}

impl Upload {
    pub fn new(data: Io, ctl: Control, conn: Arc<FtpConn>) -> Self {
        Upload { data: Some(data), ctl: Some(ctl), conn, done: None }
    }
}

fn closed() -> Error {
    Error::new(ErrorKind::BrokenPipe, "The upload has already finished")
}

impl AsyncWrite for Upload {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<std::io::Result<usize>> {
        match self.get_mut().data.as_mut() {
            Some(data) => Pin::new(data).poll_write(cx, buf),
            None => Poll::Ready(Err(closed())),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut().data.as_mut() {
            Some(data) => Pin::new(data).poll_flush(cx),
            None => Poll::Ready(Ok(())),
        }
    }

    /// Closing the data connection is what tells the server the file is
    /// complete; its reply then says whether it was stored.
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        if let Some(data) = this.data.as_mut() {
            ready!(Pin::new(data).poll_shutdown(cx))?;
            this.done = Some(finishing(this.ctl.take(), this.conn.clone(), this.data.take()));
        }
        if let Some(done) = this.done.as_mut() {
            let result = ready!(done.as_mut().poll(cx));
            this.done = None;
            return Poll::Ready(result);
        }
        Poll::Ready(Ok(()))
    }
}
