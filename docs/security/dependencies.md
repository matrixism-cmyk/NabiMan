# 백엔드 의존성 점검과 교체 계획

## 점검 방법

- 설정: `backend/deny.toml` (cargo-deny). 알려진 취약점(RustSec), 관리 중단 크레이트, 라이선스, 출처(crates.io 만)를 봅니다.
- CI: `.github/workflows/ci.yml` 의 `backend-dependencies` 작업이 `cargo deny --locked check` 를 돌립니다.
- 로컬: `cargo install cargo-deny` 후 `cd backend && cargo deny --locked check`.
  2026-11-10 기준 이 서버에는 cargo-deny/cargo-audit 가 설치되어 있지 않고 외부 네트워크로 설치할 수 없어,
  **이 설정은 아직 로컬에서 한 번도 돌려 보지 않았습니다.** 첫 CI 실행 결과로 아래 항목(특히 RUSTSEC 번호와
  라이선스 목록)을 확인해 주세요.
- 관리 중단(unmaintained) 경고는 직접 의존하는 크레이트만 실패로 칩니다(`unmaintained = "workspace"`).
  간접 의존은 아래 표로 관리하고, `ignore` 에는 kube 를 거쳐 들어오는 것만 이유와 함께 적었습니다.

## 교체가 필요한 크레이트

| 크레이트 | 버전 | 들어오는 경로 | 상태 | 계획 |
|---|---|---|---|---|
| `actix-web-actors` | 4.3.1+deprecated | 직접 (터미널·공유·VNC/RDP·시리얼·받아쓰기 WebSocket, 9개 파일) | 폐기(deprecated), actix 액터 모델에 묶임 | `actix-ws` 로 옮깁니다. 액터(`StreamHandler`/`ctx.text`) 대신 `actix_ws::handle` 이 주는 `Session`/`MessageStream` 을 tokio 작업에서 다루는 구조라 파일마다 손이 갑니다. 터미널(`pty.rs`, `mod.rs`)부터 한 번에 하나씩, 각 단계마다 WebSocket 스모크(`smoke-terminal-*`, 공유 링크)로 확인. 끝나면 `actix` 의존도 뺍니다. |
| `serde_yaml` | 0.9.34+deprecated | `kube-client` 0.95 | 폐기 | NabiMan 코드는 쓰지 않습니다. kube 를 올릴 때 사라지는지 `cargo tree -i serde_yaml` 로 확인합니다. |
| `backoff` | 0.4.0 | `kube-runtime` 0.95 | 관리 중단 (RUSTSEC-2025-0012, 번호는 첫 CI 실행으로 확인) | kube 업그레이드로 해결 (새 kube-runtime 이 다른 재시도 크레이트를 쓰는지 `cargo tree -i backoff` 로 확인). |
| `instant` | 0.1.13 | `backoff` ← `kube-runtime` | 관리 중단 (RUSTSEC-2024-0384) | `backoff` 와 함께 사라집니다. |
| `derivative` | 2.2.0 | `kube-runtime` 0.95 | 관리 중단 (RUSTSEC-2024-0388) | kube 업그레이드로 해결 (`cargo tree -i derivative` 로 확인). |
| `rustls-pemfile` | 2.2.0 | `kube-client` 0.95 | 관리 중단 (기능이 `rustls-pki-types` 로 옮겨감) | kube 업그레이드로 해결. NabiMan 코드는 직접 쓰지 않습니다. |

### kube 업그레이드 (0.95 → 최신)

위 표의 다섯 개가 모두 `kube` 0.95 에서 옵니다. 한 번에 정리하는 길은 kube 와 `k8s-openapi` 를 함께 올리는 것입니다.

1. `kube`, `k8s-openapi` 버전을 올리고 `features = ["latest"]` 를 새 버전의 쿠버네티스 버전 기능 이름으로 맞춥니다.
2. `mec/services/kube_real/` 의 API 변경(주로 `Api::*` 의 인자, `runtime::watcher` 설정)을 고칩니다.
3. `cargo test` 와 MEC 스모크(mock 모드)로 확인하고, 실제 클러스터에서는 읽기 전용 조회부터 확인합니다.
4. `deny.toml` 의 `ignore` 세 줄을 지웁니다.

이번 단계(Phase 3)에서는 바꾸지 않았습니다. 사소한 교체가 아니고, MEC 실클러스터 확인이 필요하기 때문입니다.
