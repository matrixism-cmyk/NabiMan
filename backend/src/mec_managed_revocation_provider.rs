use super::*;

struct Server {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            t.join().unwrap();
        }
    }
}
pub(super) struct Provider {
    pub address: std::net::SocketAddr,
    pub armed: Arc<AtomicBool>,
    pub exists: Arc<AtomicBool>,
    pub deletes: Arc<AtomicUsize>,
    _server: Server,
}
pub(super) fn start(
    mode: &'static str,
    db: &mec::db::DbHandle,
    users: &users::UserStore,
    sessions: &jwt_sessions::SessionStore,
) -> Provider {
    let armed = Arc::new(AtomicBool::new(false));
    let exists = Arc::new(AtomicBool::new(true));
    let deletes = Arc::new(AtomicUsize::new(0));
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let server_armed = armed.clone();
    let server_exists = exists.clone();
    let server_deletes = deletes.clone();
    let server_db = db.clone();
    let server_users = users.clone();
    let server_sessions = sessions.clone();
    let stop = Arc::new(AtomicBool::new(false));
    let server_stop = stop.clone();
    let thread = std::thread::spawn(move || {
        while !server_stop.load(Ordering::SeqCst) {
            let mut socket = match listener.accept() {
                Ok((s, _)) => s,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(3));
                    continue;
                }
                Err(e) => panic!("{e}"),
            };
            socket
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut buffer = [0; 2048];
            while !bytes.windows(4).any(|w| w == b"\r\n\r\n") {
                let n = socket.read(&mut buffer).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buffer[..n]);
            }
            let request = String::from_utf8(bytes).unwrap();
            let mut line = request.lines().next().unwrap().split_whitespace();
            let method = line.next().unwrap();
            let path = line.next().unwrap();
            let enabled = server_armed.load(Ordering::SeqCst);
            let mut status = 200;
            let body = if method == "GET" && path.starts_with("/v3/projectroletemplatebindings?") {
                if enabled && mode == "scope_revoked_during_io" {
                    server_db.lock().unwrap().execute("UPDATE tenant_access_delegations SET enabled=0 WHERE tenant_id='tenant'",[]).unwrap();
                }
                if enabled && mode == "manager_revoked_during_io" {
                    server_users.lock().unwrap()[0].memberships[0].enabled = false;
                }
                if enabled
                    && mode == "readback_failure"
                    && server_deletes.load(Ordering::SeqCst) > 0
                {
                    status = 503;
                    "provider-sensitive-marker".into()
                } else {
                    let owned = if server_exists.load(Ordering::SeqCst) {
                        format!(
                            r#"{{"id":"owned-one","projectId":"{}","userId":"{}","roleTemplateId":"read-only"}},"#,
                            if enabled && mode == "wrong_project" {
                                "c:other"
                            } else {
                                "c:p"
                            },
                            if enabled && mode == "wrong_identity" {
                                "different-user"
                            } else {
                                "remote-member"
                            }
                        )
                    } else {
                        String::new()
                    };
                    format!(
                        r#"{{"data":[{owned}{{"id":"foreign-one","projectId":"c:p","userId":"unrelated-user","roleTemplateId":"project-owner"}}],"pagination":{{"partial":false}}}}"#
                    )
                }
            } else if method == "DELETE" && path == "/v3/projectroletemplatebindings/owned-one" {
                assert!(enabled);
                server_deletes.fetch_add(1, Ordering::SeqCst);
                if mode == "accepted_delete" {
                    status = 202
                } else {
                    server_exists.store(false, Ordering::SeqCst);
                }
                if mode == "confirmation_audit_failure" {
                    server_db.lock().unwrap().execute_batch("CREATE TRIGGER reject_revoke_confirm BEFORE INSERT ON audit_logs WHEN NEW.action='tenant_member_delete_confirmed' BEGIN SELECT RAISE(ABORT,'confirmation audit fault'); END;").unwrap();
                }
                if mode == "caller_logout" {
                    server_sessions.lock().unwrap().remove("manager-sid");
                }
                if mode == "lost_delete" {
                    continue;
                }
                "{}".into()
            } else {
                panic!("Unexpected provider method/path: {method} {path}")
            };
            let response=format!("HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());
            let _ = socket.write_all(response.as_bytes());
        }
    });
    let _server = Server {
        stop,
        thread: Some(thread),
    };
    Provider {
        address,
        armed,
        exists,
        deletes,
        _server,
    }
}
