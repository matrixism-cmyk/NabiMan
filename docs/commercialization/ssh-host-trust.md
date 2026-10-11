# 서버 키 신뢰와 교체 운영

이 문서는 AXGate와 일반 원격 서버에 구현한 서버 키 검증의 설정·이전 절차다. 일반 원격 서버에는 공개키·독립 확인한 지문·확인 사유를 등록하는 화면과 API를 추가했고 OpenSSH 접속 경로의 accept-new를 제거했다. 장비 실측·운영 rollout·변경의 독립 감사·진행 중인 접속 회수는 후속 범위다. 운영 서비스에 자동 배포하거나 기존 장비의 키를 자동 신뢰하지 않는다.

2026-10-05 23:48 KST의 명시적 운영 배포에서 일반 서버 6개를 이전했다. root의 기존 known_hosts에서 검토한 키를 사용했고 5개 서버는 실제 keyscan 일치를, syc.insnetkorea.com은 기존 호환 설정과 StrictHostKeyChecking=yes의 무암호 handshake로 RSA 키 일치를 확인했다. 새로 관측한 키를 자동 승인하지 않았다. 배포 후 syc에서는 새 고정 키 경로와 기존 암호화 credential로 실제 SSH 인증·무해한 printf 확인 명령까지 성공했다. 서버 이름/대상·신뢰 generation과 기존 terminal binding은 배포 계획에 고정했다. 이는 해당 6개 서버의 이전 증거이며 전체 장비 matrix·AXGate rollout·legacy 예외 종료·독립 감사의 완료가 아니다.

## 검증 동작

AXGate 접속은 등록된 SHA256 서버 공개키 지문과 실제 handshake에서 제시된 키가 일치할 때만 비밀번호 인증을 수행한다. 설정 누락·잘못된 형식은 socket 연결 전에 거부하고, 키 불일치는 비밀번호 전달 전에 차단한다. 지문은 OpenSSH의 SHA256 형식인 `SHA256:` 접두어와 43자 unpadded base64 digest로 제한한다. 공개키의 지문은 비밀정보가 아니지만 연결 식별 정보이므로 필요한 관리자에게만 제공한다.

OpenSSH는 새 키를 신뢰하는 정책과 변경된 키 거부를 구분한다. accept-new는 최초 키를 자동 등록하므로 별도의 독립 확인과 같지 않다. 이번 AXGate 구현은 사전 확인한 키를 요구한다. [OpenSSH 설정](https://man.openbsd.org/ssh_config)

## 최초 키 등록

1. 장비의 콘솔 또는 이미 검증된 관리 경로에서 서버 공개키·SHA256 지문을 확보한다. 장비가 제공하는 export·확인 기능을 사용하고 개인키를 복사하지 않는다.
2. 공개키 파일을 확보한 경우 `ssh-keygen -l -E sha256 -f axgate-host-key.pub`로 지문을 확인한다. 장비 화면의 지문과도 대조한다. [ssh-keygen 문서](https://man.openbsd.org/ssh-keygen)
3. 관리자 배포 설정에 `NABIMAN_MEC_AXGATE_HOST_KEY_SHA256=SHA256:<검증한 지문>`을 추가한다. 꺾쇠 괄호와 안내 문구는 실제 설정에 넣지 않는다.
4. 변경 사유, 장비 식별자·host·port, 확인 경로·확인자, 지문과 적용 시각을 변경 기록에 남긴다. 배포 설정 파일은 운영 관리자만 수정할 수 있게 한다.
5. 관리 서비스를 계획된 시간에 재시작하고 health에서 실제 접속 결과를 확인한다. 실패하면 원인을 확인하며 검증을 끄거나 mock으로 우회하지 않는다.

ssh-keyscan만으로 받은 키는 네트워크에서 누가 제시했는지 독립적으로 증명하지 않는다. 얻은 지문을 경고 화면에서 그대로 복사해 등록하는 방식은 키 교체 검증을 대신하지 않는다.

## 오래된 알고리즘의 임시 예외

기본 연결은 russh의 기본 알고리즘을 사용한다. AXGate가 SHA1 기반의 구형 KEX 또는 ssh-rsa를 반드시 요구하는 경우에만 `NABIMAN_MEC_AXGATE_LEGACY_UNTIL`에 RFC3339 만료 시각을 설정한다. 만료는 적용 시점 이후 최대 30일 안이어야 한다. 예외는 AXGate connector의 단일 host 설정에만 적용되고 서버 키 검증을 해제하지 않는다.

승인자는 장비 교체·firmware upgrade의 담당과 일정을 확인하고 예외 사유·기간을 기록한다. 기한이 지나거나 잘못된 값이면 신규 접속을 거부한다. 기존 session을 실시간 강제 종료하는 기능은 이번 구현에 포함하지 않았으며 후속 session 회수 경로에서 다룬다. 캐시를 반환하기 전에도 현재 예외 설정의 유효성을 검사한다.

## 서버 키 교체

장비 재설치·교체·정상적인 키 회전에도 기존 지문과 달라지면 연결이 차단된다. 먼저 변경이 예정된 작업인지, 대상 host·port가 맞는지, 콘솔·검증된 관리 경로의 새 키가 일치하는지 확인한다. 승인된 새 지문으로 배포 설정을 교체하고 재시작한다. 기존 키와 새 키를 자동으로 함께 신뢰하거나 실패한 접속의 키를 자동 승인하지 않는다.

긴급 변경도 검증 가능한 별도 경로와 변경 기록을 유지한다. 이전 키로 되돌릴 때는 장비의 실제 키도 일치해야 하며, 새 키를 유지한 장비에 설정만 되돌리면 접속이 차단되는 것이 정상이다.

## 검증과 남은 범위

로컬 검증용 SSH 서버에서 일치하는 키의 비밀번호 인증, 다른 키의 인증 전 차단, 신뢰 설정 누락 시 socket 연결 없음, 기본 알고리즘으로 구형 서버 거부, 만료 전 예외의 구형 서버 접속을 시험한다. 잘못된 지문·만료·30일 초과 예외도 거부한다. 실제 AXGate 장비와 production 재접속 검증은 별도 rollout 단계다.

일반 서버와 terminal·SSH key 배포의 trust 저장 및 관리자 확인 화면은 아래 후속 구현 절차를 참조한다. 남은 개발은 교체 승인 workflow와 감사 event, configuration hot reload, live session 회수, host별 legacy 예외 관리다. 이 문서의 수동 운영 절차만으로 C03 전체 완료를 선언하지 않는다.

## 일반 원격 서버 등록과 교체

최고관리자는 원격 화면에서 서버를 선택한 뒤 설정의 서버 신원 확인을 사용한다. 새 서버를 추가하면 설정 화면으로 이동한다. 소속 서버 관리자는 내 소속 서버의 서버 신원 확인 버튼을 사용한다. 조회자·운영자는 키를 변경할 수 없고 다른 소속 관리자에게도 대상 서버의 등록 권한을 주지 않는다.

서버 공개키 한 줄, 콘솔 등 별도 경로에서 확인한 SHA256 지문, 확인 경로·교체 사유를 입력한다. 서버는 공개키를 파싱하고 지문을 계산해 입력한 지문과 일치하는지 검증한다. 등록 시 확인자·UTC 시각·사유를 기록한다. 키 교체·해제는 이전 지문·승인 version과 확인 당시 host·port가 일치해야 하며 다른 관리자가 키나 대상을 바꾸면 409로 거부한다. 교체 전에 새 키를 독립 확인해야 한다. UI가 직접 관측한 키를 자동 승인하지 않는다.

키 신뢰를 해제하면 신규 접속에 필요한 known_hosts 옵션을 만들지 못해 접속을 거부한다. host·port를 편집해도 기존 신뢰를 해제하며 새 대상을 다시 확인해야 한다. 최근 온라인 상태도 초기화한다. 확인한 공개키로 관리 데이터 경로에 0600 known_hosts 파일을 생성하고 상태 확인·명령 실행·terminal·SSH 사용자 키 배포·키 인증 시험에 같은 StrictHostKeyChecking=yes 정책을 적용한다. global known_hosts·DNS·KnownHostsCommand·자동 key update로 다른 신뢰 경로를 사용하지 않는다. 공개키 version별 파일을 사용하므로 기존 세션이 읽는 파일을 새 키로 덮어쓰지 않는다.

직접 host·port·user를 지정하는 terminal도 같은 대상이 저장되고 키가 등록된 경우에만 허용한다. 기존 JSON에 키 신뢰가 없으면 미확인 상태로 읽으며 기존 global known_hosts나 최초 접속 키를 자동으로 import하지 않는다. 배포 전 기존 대상의 독립 확인·등록을 준비하고 rollout 중 미확인 대상의 새 접속은 차단되는 것을 안내한다. 관리 대상의 workload를 중지하지 않는다.

브라우저가 붙은 원격 터미널은 아래 승인 snapshot 검사로 회수한다. 이미 전달된 SSH 명령·tmux/원격 process 자체와 일반 probe/exec의 진행 중 요청을 강제 종료하는 기능은 아직 없다. 계정·정책·trust version과 live session·job을 연결한 재인가·회수는 C09·C10에서 검증한다. 확인자·사유의 최신 기록은 저장하지만 이전·이후 키와 승인 절차의 독립적 append only 감사는 C11의 후속 범위다. version 파일의 보존·정리와 전체 legacy 알고리즘 정책도 별도로 완성해야 한다.

## 승인 generation과 오래된 화면의 변경 차단

일반 서버의 `host_trust_version`은 UUID다. 새 서버에 생성하고 키 등록·같은 키 재승인·교체·해제 때마다 새 값으로 교체한다. host/port 변경으로 신뢰를 해제할 때도 교체하며, 신뢰가 없는 상태에도 값을 보존한다. 같은 공개키와 같은 지문으로 돌아와도 과거 승인의 version은 다시 쓰지 않는다.

관리 화면/API는 현재 조회한 version을 `previous_version`으로 제출한다. 서버 write mutex 안에서 현재 host·port·지문·version을 비교한 뒤 기록한다. 하나라도 다르면 409이며 자동 재시도하지 않고 현재 상태를 조회해 새 변경을 검토한다. 최초 신뢰 없음→등록→해제→신뢰 없음으로 돌아온 경우에도 이전 빈 지문의 요청을 허용하지 않는다. [RFC 9110의 조건부 변경](https://www.rfc-editor.org/rfc/rfc9110.html#section-13.1.1)과 같은 충돌 방지 목적의 application CAS이며 HTTP If-Match/ETag protocol을 구현한 것은 아니다.

상태 점검 결과도 시작 당시 승인 version과 최신 version이 같을 때만 저장한다. 같은 키를 재승인한 뒤 과거 점검의 online 결과가 새 승인의 검증 상태를 덮어쓰지 못한다. 표시 이름·memo 변경은 기존 승인 version을 유지한다.

기존 JSON의 누락 version은 빈 값으로 읽는다. 현재 값이 빈 경우에만 빈 `previous_version`으로 첫 명시적 변경을 허용하고 새 UUID를 저장한다. 기존 검증된 키를 자동 해제하거나 최신 generation으로 추정해 채우지는 않는다. 새/변경된 record에 구버전 UI가 version 없이 요청하면 409로 거부하므로 frontend/backend를 함께 교체해야 한다. 이 generation을 아래 terminal snapshot 경계에 연결했다. 일반 진행 중 probe/exec 요청 회수·transaction 감사·HA·장비 rollout은 후속 작업이다.

## 원격 터미널 attachment의 진행 중 회수

새 원격 터미널은 saved server ID·host·port·SSH user·auth method·canonical key·fingerprint·승인 generation을 `remote_binding`에 저장한다. saved server 선택과 직접 host/port/user 선택 모두 실제 명령을 만들 때 읽은 동일 record에서 binding을 만든다. 명령 생성 뒤와 tmux 생성 전에 현재 승인과 계정 grant를 다시 확인한다. legacy server에 승인 generation이 없으면 키를 다시 독립 확인해 명시적으로 승인한 뒤 새 원격 세션을 열어야 한다.

account attachment는 현재 session/tmux tuple와 remote binding을 검사한다. 공유 attachment도 같은 remote binding을 기존 ShareGrant 검사에 포함한다. 시작·각 입력·output frame·history/geometry·유휴 검사에서 현재 remote record의 ID·대상·SSH user·auth method·key/fingerprint·generation이 모두 일치해야 한다. 조회 오류·server 삭제·trust 해제·교체·같은 키 재승인·대상 변경은 거부하고 진행 중 연결을 1008로 종료한다. 정책 종료에 기존 frontend의 자동 재접속 차단을 유지한다.

이전 generation으로 이미 만든 session은 같은 key 재승인 후 자동 갱신하지 않는다. 재접속에는 원래 binding을 확인하고, 승인 변경 시 종료 안내만 보내 tmux attach와 다른 local shell 생성으로의 fallback을 하지 않는다. 목록의 preview와 REST scrollback도 유효 승인이 있어야 제공한다. 사용 불가 session은 목록에 alive=false·빈 preview로 남아 최고관리자가 정리할 수 있다. 공개 공유 metadata는 remote admission이 무효이면 alive=false·빈 geometry이며 새 attachment는 허용하지 않는다. password endpoint가 새 입장권을 발급해도 마지막 remote 검사에서 접속을 거부한다.

binding을 저장해 정상 복원한 원격 session도 같은 검사 대상이다. legacy session에서 원격 binding을 이름·명령으로 추정하거나 현재 승인을 붙이지 않는다. binding이 없는 session은 ssh_cmd가 없고 label이 정확히 local인 경우만 local로 인정한다. 기존 원격/알 수 없는 session은 새로 열어야 한다. 그 session의 tmux/원격 명령을 자동으로 죽이는 migration은 아니다.

현재 검사마다 단일 서버의 remote registry를 읽고 검증한다. correctness 경계의 구현이며 대규모 terminal 처리량·검사 지연 SLA를 실측한 것은 아니다. 진행 중 OS 명령·원격 SSH connection·백그라운드 작업·이미 전달한 화면과 큐에 든 frame을 소급 회수하지 않는다. session metadata에는 [단일 writer와 원자 snapshot 저장](terminal-registry.md)을 적용했다. 검사와 OS IO 사이 원자 cutoff, general probe/exec/JIT job 재인가, metadata·OS 작업·감사 사이 트랜잭션·HA·clock·복구 인가는 후속 범위다. OS 프로세스 격리와 scoped terminal 권한 확대도 별도 특권 분리 단계에 유지한다.
