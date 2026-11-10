# 이 서버(나비맨 운영 서버)가 지금처럼 동작하기 위한 설정

새 빌드는 이 서버만의 값을 더 이상 기본으로 갖지 않습니다([설치 환경 설정](configuration.md)). 새 빌드를 배포하기 **전에** 아래 파일을 만들어 두면 사용자가 보기에 달라지는 것이 없습니다. 비밀은 이 문서에 없고, 지금 있는 곳에 그대로 둡니다.

## 파일 위치

이 서버는 컨테이너라 다시 시작하면 `/etc`가 초기화됩니다. 그래서 `/etc/nabiman/nabiman.toml`이 아니라 데이터 디렉터리의 파일을 씁니다(시작 스크립트가 `NABIMAN_DATA_DIR=/workspace/var/lib/nabiman`을 줍니다).

```
/workspace/var/lib/nabiman/nabiman.toml   (root:root, 0600 또는 0644)
```

만드는 방법(서버를 멈출 필요 없음, 다음 시작부터 읽힘):

```
install -o root -g root -m 0600 nabiman.toml /workspace/var/lib/nabiman/nabiman.toml
```

## 내용

```toml
# 이 서버의 설치 설정 (docs/operations/this-box-config.md)

# Office·한글 미리보기: /workspace/opt 에 둔 LibreOffice(H2Orestart 포함), rhwp, Noto KR 글꼴
soffice = "/workspace/opt/libreoffice-26.2.7/opt/libreoffice26.2/program/soffice.bin"
rhwp = "/workspace/opt/rhwp-0.8.7/bin/rhwp"
preview_fonts = "/workspace/opt/fonts-noto-kr"

# Claude 음성 대화: 서버의 PATH에 없는 nabi 계정의 npm 전역 설치
claude = "/workspace/.npm-global/bin/claude"

# 받아쓰기: dictation.json 에 이미 https://v.bebo.kr 이 저장돼 있어 없어도 되지만,
# 저장값이 지워져도 같은 서비스를 쓰도록
dictation_url = "https://v.bebo.kr"

# 새 Let's Encrypt 인증서 (SSL 화면, 가상 호스트 자동 SSL)
letsencrypt_email = "webmaster@xos.kr"

# DNS 화면 상태 점검
dns_check_domain = "xos.kr"
dns_check_expect = "115.68.193.237"

# MEC LB 주소 풀 (예전 코드에 있던 두 풀)
mec_lb_pools = ["tenant=172.20.26.151-172.20.26.200 auto", "reserved=172.20.26.231-172.20.26.238"]

# MEC NOC 경보 띠의 당직 연락처
noc_contact = "the@aeokorea.com"

# 개선 리포트 위젯 (fix.bebo.kr, 관리자에게만 보임)
feedback_origin = "https://fix.bebo.kr"
feedback_site = "fs_4oAGj2jt6Dgzp61MFo0uTq7_"
```

`feedback_site`는 비밀이 아닙니다(예전에도 `fixhub-admin.js`로 누구에게나 내려가던 값).

## 그대로 두는 것

- 시작 스크립트 `/workspace/nabiman/bin/start-nabiman.sh`가 주는 환경 변수(`NABIMAN_PORT=18080`, `NABIMAN_STATIC=/workspace/nabiman/static`, `NABIMAN_DATA_DIR=/workspace/var/lib/nabiman`, `NABIMAN_SERIAL_GROUP=nabi`)와 `PATH`(tmux 3.8, `/workspace/opt/bin`의 sshpass·guacd). 환경 변수가 파일보다 우선하므로 겹쳐도 문제없습니다.
- guacd 주소는 기본값 `127.0.0.1:4822` 그대로입니다.
- 듣는 주소는 기본값(`127.0.0.1:18080`) 그대로 둡니다. nginx의 `man.bebo.kr`이 `proxy_pass http://127.0.0.1:18080`으로 프록시하고, 2026-11-11에 18080으로 들어온 연결은 모두 127.0.0.1에서 온 것이었으며, 18080을 쓰는 k8s Service·LoadBalancer·스크립트는 없었습니다. 예전 기본(`0.0.0.0`)에서는 같은 클러스터의 다른 파드가 nginx를 거치지 않고 18080에 닿을 수 있었습니다.

## 비밀이 있는 곳 (옮기지 않음)

| 비밀 | 있는 곳 |
|---|---|
| 첫 관리자 비밀번호(`NABIMAN_PASSWORD`) | `/workspace/nabiman/secrets/admin_password` — 시작 스크립트가 읽어 환경 변수로 넘김 |
| 저장 비밀번호 암호화 키 | 데이터 디렉터리 `secret.key` |
| 받아쓰기 API 키 | 데이터 디렉터리 `dictation.json`(위 키로 암호화) |
| nabidns DB 접속 정보 | 데이터 디렉터리 `nabidns-mysql.cnf` |
| MEC Rancher·Harbor·AXGATE 토큰·비밀번호 | 지금처럼 환경 변수(설정 파일에 넣지 않음) |

## 배포 뒤 확인

- 시작 로그에 `Settings: /workspace/var/lib/nabiman/nabiman.toml (12 from the file, 0 overridden by the environment)`과 `External programs: …`가 나옵니다. `not available:`에 `soffice`, `rhwp`, `claude`가 없어야 합니다.
- 진단 화면 "외부 프로그램" 표에서 모두 "있음"인지(ffmpeg 제외) 확인합니다.
- 관리자 화면 오른쪽 아래 "개선 리포트 (외부)" 버튼, DNS 화면 상태 "정상", MEC 저장소 화면의 LB 풀 2개, NOC 화면의 당직 연락처가 그대로 보이는지 확인합니다.
- `scripts/deploy-reviewed-runtime.py`는 이제 위치를 인자로 받습니다: `--root /workspace/nabiman --data-dir /workspace/var/lib/nabiman --supervisor-conf /workspace/etc/supervisor/supervisord.conf --local-url http://127.0.0.1:18080 --public-url https://man.bebo.kr` (또는 같은 이름의 `NABIMAN_DEPLOY_*` 환경 변수).
