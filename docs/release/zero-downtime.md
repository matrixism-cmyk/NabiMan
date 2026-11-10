# 무중단 배포: 현재 상태와 설계

2026-11-10 기준 검토입니다. 결론부터: **지금 구조로는 두 프로세스를 겹쳐 띄우는 진짜 무중단은 불가능**하고, 현실적인 목표는 "재시작 동안 연결 거부 없음 + 끊긴 터미널이 몇 초 안에 같은 세션으로 다시 붙음"입니다. 이번 작업(Phase 7)에서는 코드를 바꾸지 않았습니다. 필요한 변경이 `backend/src/main.rs`와 터미널 WebSocket 처리부에 걸쳐 있고 그쪽은 다른 작업이 진행 중이라, 아래 설계로 남깁니다.

## 지금 재시작하면 일어나는 일

1. `systemctl restart nabiman`(또는 `nabiman-update`)이 SIGTERM을 보냅니다.
2. actix-web은 기본 graceful shutdown으로 새 연결을 받지 않고, 진행 중인 연결이 끝나기를 **최대 30초**(`shutdown_timeout` 기본값) 기다립니다. 열린 터미널·VNC WebSocket도 "진행 중"이라, 브라우저에 터미널 창이 하나라도 열려 있으면 이전 프로세스는 거의 항상 30초를 다 채우고 나서야 끝납니다.
3. 그동안 포트는 아무도 받지 않으므로(이전 프로세스는 accept를 멈췄고 새 프로세스는 아직 없음) 새 요청은 **연결 거부**됩니다. 웹 화면 새로고침, API 호출, 재접속 시도가 모두 실패합니다.
4. 이전 프로세스가 끝나야 새 프로세스가 시작할 수 있습니다. 데이터 디렉터리의 `.lock`(flock, `backend/src/data_dir/lock.rs`)과 MEC DB 잠금이 한 번에 한 서버만 허용하기 때문입니다. JSON 저장소가 메모리 내용을 파일 전체로 다시 쓰는 구조라, 두 서버가 겹치면 서로의 변경을 지웁니다.
5. 새 프로세스가 뜨면 브라우저의 터미널(`TerminalView`)이 자동 재연결하고, tmux 세션이 그대로 있으므로 같은 화면으로 돌아옵니다.

터미널 세션 자체는 tmux가 지키므로 **작업 내용은 잃지 않습니다**. systemd 설치에서는 이번에 unit에 `KillMode=process`를 넣었습니다. 기본값(`control-group`)이었다면 재시작마다 tmux 서버까지 같은 cgroup에서 죽어 모든 세션이 사라졌습니다. 이 개발 서버는 supervisord 아래에서 돌고 tmux 서버가 이미 독립 데몬이라 해당하지 않습니다.

즉 지금의 중단 시간은 "이전 프로세스 종료 대기(열린 WebSocket이 있으면 약 30초) + 새 프로세스 기동"이고, 그 사이 요청은 거부됩니다.

## 검토한 방법

| 방법 | 효과 | 막는 것 |
|---|---|---|
| SO_REUSEPORT로 새 프로세스를 먼저 띄우고 이전 것을 drain | 연결이 하나도 끊기지 않음(이상적) | 데이터 디렉터리 잠금: 두 서버가 동시에 돌 수 없음. 저장소 소유권을 한 프로세스로 모으거나(예: 저장소 데몬) 파일 단위 잠금·재적재로 바꾸는 큰 구조 변경이 먼저 필요 |
| 실행 중 소켓 넘기기(fd passing, 예: 이전 프로세스가 새 프로세스를 exec) | 리스닝 소켓 유지 | 위와 같은 이유로 새 프로세스는 이전 것이 잠금을 놓은 뒤에야 저장소를 열 수 있음. 결국 아래 방법과 효과가 같고 더 복잡 |
| **systemd 소켓 활성화** + 짧은 종료 + WebSocket 종료 알림 | 재시작 동안 연결 거부 없음(커널이 대기열에 보관), 중단 시간이 30초대에서 1~2초대로 | 작은 코드 변경과 unit 하나 |

## 권하는 설계 (작고 안전한 순서)

### 1. 종료를 짧게 (main.rs 한 줄)

```rust
HttpServer::new(...)
    .shutdown_timeout(3)   // 기본 30초: 열린 WebSocket이 재시작을 30초 붙잡음
```

일반 HTTP 요청은 3초면 충분합니다(긴 다운로드·업로드만 끊깁니다). WebSocket은 어차피 끊기고 재연결하므로 오래 기다릴 이유가 없습니다.

### 2. 끊기 전에 알리기 (터미널·VNC WebSocket 처리부)

SIGTERM을 받으면 열린 WebSocket에 close 코드 **1012(Service Restart)** 를 보냅니다. actix의 `ServerHandle`/시그널 처리에서 전역 `tokio::sync::watch`(또는 `CancellationToken`)를 울리고, 각 WebSocket 세션 actor가 이를 받아 `ctx.close(Some(CloseCode::Restart.into()))`를 호출합니다. 브라우저는 1012를 받으면 "서버 업데이트 중"을 보여 주고 바로 재연결을 시작합니다(지금 재연결 로직의 대기 시간을 1012일 때만 짧게). 이것으로 이전 프로세스는 3초를 다 기다리지 않고 거의 즉시 끝납니다.

### 3. systemd 소켓 활성화 (main.rs 약 15줄 + `packaging/nabiman.socket`)

```ini
# /etc/systemd/system/nabiman.socket
[Socket]
ListenStream=8080
NoDelay=true
Backlog=1024
[Install]
WantedBy=sockets.target
```

```rust
// systemd가 넘긴 소켓(fd 3)이 있으면 그것을, 없으면 지금처럼 bind.
fn inherited_listener() -> Option<std::net::TcpListener> {
    use std::os::fd::FromRawFd;
    let pid: u32 = std::env::var("LISTEN_PID").ok()?.parse().ok()?;
    let fds: i32 = std::env::var("LISTEN_FDS").ok()?.parse().ok()?;
    if pid != std::process::id() || fds < 1 { return None; }
    std::env::remove_var("LISTEN_PID"); std::env::remove_var("LISTEN_FDS"); // 자식(tmux 등)에 넘기지 않음
    let l = unsafe { std::net::TcpListener::from_raw_fd(3) };
    l.set_nonblocking(true).ok()?;
    Some(l)
}
// ...
let server = HttpServer::new(...).shutdown_timeout(3);
let server = match inherited_listener() {
    Some(l) => server.listen(l)?,
    None => server.bind(format!("0.0.0.0:{}", port))?,
};
server.run().await
```

fd 3에는 close-on-exec를 걸어 tmux·셸이 물려받지 않게 합니다(`fcntl(3, F_SETFD, FD_CLOEXEC)`). 포트는 `nabiman.socket`이 정하므로 `NABIMAN_PORT`와 어긋나지 않게 설치 스크립트가 함께 씁니다(override: `systemctl edit nabiman.socket`).

이렇게 하면 리스닝 소켓은 systemd가 계속 들고 있고, 재시작하는 1~2초 동안 들어온 연결은 거부되지 않고 커널 대기열에서 새 프로세스를 기다립니다. 사용자에게는 "잠깐 느린 요청"으로 보입니다. 소켓 없이 실행하면(개발 서버, 컨테이너, supervisord) 지금처럼 직접 bind하므로 기존 배포에 영향이 없습니다.

### 4. nabiman-update 쪽

`nabiman-update`는 이미 재시작 후 MainPID의 실행 파일과 응답을 확인하고, 실패하면 되돌립니다. 소켓 활성화가 들어오면 상태 확인 동안에도 포트가 열려 있어, 확인 요청이 연결 거부 대신 새 프로세스의 응답을 기다립니다. 추가 변경은 없습니다(`--health-timeout`은 그대로 의미가 있습니다).

## 남는 한계

- 열린 WebSocket은 여전히 한 번 끊깁니다(1012 후 자동 재연결, tmux가 화면 복원). 연결을 끊지 않으려면 저장소 잠금 구조를 바꾼 뒤 SO_REUSEPORT 교대 방식으로 가야 합니다. 상용화 단계에서 필요해지면 저장소 소유를 한 프로세스(또는 SQLite 트랜잭션)로 옮기는 작업과 묶어 다시 검토합니다.
- 진행 중인 긴 HTTP 요청(큰 파일 업로드·다운로드)은 3초 뒤 끊깁니다. 업데이트는 사용이 적은 시간에 합니다.
- 컨테이너는 이미지 교체가 곧 프로세스 교체라 위 방법이 해당하지 않습니다. 앞단 프록시의 재시도로 보완합니다.

## 확인 방법 (구현 후)

1. 터미널 창 두 개를 연 채로 `nabiman-update`를 실행합니다.
2. 다른 창에서 `while true; do curl -s -o /dev/null -w '%{http_code} %{time_total}\n' http://127.0.0.1:8080/; sleep 0.2; done`로 응답 코드를 봅니다. 연결 거부(`000`)가 없어야 하고, 지연이 재시작 시간(1~2초) 안이어야 합니다.
3. 두 터미널이 "서버 업데이트 중" 표시 후 같은 세션으로 돌아오는지 봅니다.
