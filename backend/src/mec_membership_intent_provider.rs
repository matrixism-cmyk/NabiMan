use super::*;

struct ServerGuard {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Drop for ServerGuard {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}
pub(super) struct Provider {
    pub address: std::net::SocketAddr,
    pub restore_source: Arc<AtomicBool>,
    pub revoke_on_inventory: Arc<AtomicBool>,
    pub writes: Arc<Mutex<Vec<String>>>,
    pub provider_exists: Arc<AtomicBool>,
    _guard: ServerGuard,
}
pub(super) fn start(
    mode: &'static str,
    db: &mec::db::DbHandle,
    users: &users::UserStore,
    sessions: &jwt_sessions::SessionStore,
) -> Provider {
    let restore_source = Arc::new(AtomicBool::new(false));
    let server_restore_source = restore_source.clone();
    let server_users = users.clone();
    let server_db = db.clone();
    let server_sessions = sessions.clone();
    let revoke_on_inventory = Arc::new(AtomicBool::new(false));
    let server_revoke_on_inventory = revoke_on_inventory.clone();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let server_stop = stop.clone();
    let writes = Arc::new(Mutex::new(Vec::<String>::new()));
    let server_writes = writes.clone();
    let provider_exists = Arc::new(AtomicBool::new(false));
    let server_exists = provider_exists.clone();
    let thread = std::thread::spawn(move || {
        while !server_stop.load(Ordering::SeqCst) {
            let mut socket = match listener.accept() {
                Ok((s, _)) => s,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(5));
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
                let count = socket.read(&mut buffer).unwrap();
                assert!(count > 0);
                bytes.extend_from_slice(&buffer[..count]);
            }
            let request = String::from_utf8(bytes).unwrap();
            let line = request.lines().next().unwrap();
            let mut parts = line.split_whitespace();
            let method = parts.next().unwrap();
            let path = parts.next().unwrap();
            let mut status = 200;
            let binding = r#"{"id":"binding-one","projectId":"c:p","userId":"u-one","roleTemplateId":"read-only"}"#;
            let body = if path == "/v3/users/u-one" && method == "GET" {
                format!(
                    "{{\"id\":\"u-one\",\"username\":\"alice\",\"enabled\":{}}}",
                    mode != "managed_disabled_remote"
                )
            } else if path == "/v3/users" && method == "GET" {
                r#"{"data":[{"id":"u-one","username":"alice","enabled":true}]}"#.to_string()
            } else if path.starts_with("/v3/projectroletemplatebindings?") && method == "GET" {
                if mode == "managed_delegated_scope_revoked" {
                    server_db.lock().unwrap().execute("UPDATE tenant_access_delegations SET enabled=0 WHERE tenant_id='tenant'",[]).unwrap();
                }
                if mode == "managed_delegated_manager_revoked" {
                    server_users.lock().unwrap()[0].memberships[0].enabled = false;
                }
                if server_restore_source.load(Ordering::SeqCst) {
                    server_users.lock().unwrap()[1].memberships[0].enabled = true;
                }

                if server_revoke_on_inventory.load(Ordering::SeqCst) {
                    server_sessions.lock().unwrap().remove("sid");
                }
                if server_exists.load(Ordering::SeqCst) && mode == "readback_failure" {
                    status = 503;
                    "provider-sensitive-marker".into()
                } else {
                    format!(
                        "{{\"data\":[{}],\"pagination\":{{\"total\":{},\"partial\":false}}}}",
                        if server_exists.load(Ordering::SeqCst) {
                            binding
                        } else {
                            ""
                        },
                        if server_exists.load(Ordering::SeqCst) {
                            1
                        } else {
                            0
                        }
                    )
                }
            } else if path == "/v3/projectroletemplatebindings" && method == "POST" {
                server_writes.lock().unwrap().push("create".into());
                server_exists.store(true, Ordering::SeqCst);
                if mode == "lost_create" || mode == "managed_lost_create" {
                    continue;
                }
                if mode == "managed_tenant_changed" {
                    server_db
                        .lock()
                        .unwrap()
                        .execute(
                            "UPDATE tenant_profiles SET status=?1 WHERE id='tenant'",
                            rusqlite::params![
                                serde_json::to_string(&TenantStatus::Terminating).unwrap()
                            ],
                        )
                        .unwrap();
                }
                if mode == "managed_source_revoked" {
                    server_users.lock().unwrap()[1].memberships[0].enabled = false;
                }
                if mode == "managed_audit_failure" || mode == "managed_auto_cleanup" {
                    server_db.lock().unwrap().execute_batch("CREATE TRIGGER reject_managed BEFORE INSERT ON audit_logs WHEN NEW.action LIKE '%_managed_confirmed' BEGIN SELECT RAISE(ABORT,'fixture managed audit failure'); END;").unwrap();
                }
                if mode == "caller_logout" {
                    server_sessions.lock().unwrap().remove("sid");
                }
                if mode == "confirmation_audit_failure" {
                    server_db.lock().unwrap().execute_batch("CREATE TRIGGER reject_confirm BEFORE INSERT ON audit_logs WHEN NEW.action LIKE '%_confirmed' BEGIN SELECT RAISE(ABORT,'fixture confirmation audit failure'); END;").unwrap();
                }
                status = 201;
                if mode == "wrong_identity" {
                    r#"{"id":"foreign","projectId":"c:foreign","userId":"other-user","roleTemplateId":"project-owner"}"#.into()
                } else {
                    binding.into()
                }
            } else if path == "/v3/projectroletemplatebindings/binding-one" && method == "DELETE" {
                server_writes.lock().unwrap().push("delete".into());
                if mode == "accepted_delete" || mode == "automatic_delete_unknown" {
                    status = 202;
                } else {
                    server_exists.store(false, Ordering::SeqCst);
                }
                "{}".into()
            } else {
                panic!("Unexpected fixture method/path: {method} {path}")
            };
            let response=format!("HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());
            let _ = socket.write_all(response.as_bytes());
        }
    });
    let _guard = ServerGuard {
        stop,
        thread: Some(thread),
    };
    Provider {
        address,
        restore_source,
        revoke_on_inventory,
        writes,
        provider_exists,
        _guard,
    }
}
