# 상용 라이선스 서명과 발급 분리

2026-10-05 개발 구현. 같은 날 23:48 KST 해당 검증 코드를 포함한 binary가 man.bebo.kr에 반영됐다. 실제 유료 발급·상품 한도 집행·고객 이전이나 전체 C04 완료를 의미하지 않는다.

## 검증 경계

기존 서버 내장 HMAC 발급 비밀값과 unsigned `LicenseInfo` 저장을 제거했다. 서버는 외부 구성한 Ed25519 공개키만 읽고 발급 개인키를 갖지 않는다. `/api/license`와 `/api/license/features`는 저장된 토큰의 서명·발급자·product·필드·등급·서버 한도·feature allowlist·발급일·만료일을 매번 확인한다. 정상 서명된 payload에 있는 feature와 한도를 그대로 반환하며 등급 이름으로 추가 feature를 부여하지 않는다.

서명 검증은 `ed25519-dalek 2.2.0`의 `verify_strict`를 사용하고 weak 공개키와 중복 key ID를 구성 단계에서 거부한다. scalar와 point malleability 검사의 범위는 [공식 verify_strict 문서](https://docs.rs/ed25519-dalek/2.2.0/ed25519_dalek/struct.VerifyingKey.html#method.verify_strict)에 근거한다. FIPS 인증이나 별도 암호 감사를 완료했다는 의미는 아니다.

`POST /api/license/activate`는 JWT 서명·session·현재 회원 기록에서 확인한 최고관리자만 사용할 수 있다. 잘못된 토큰은 400, 공개키 구성 누락·손상은 503, 저장 실패는 500이며 기존 정상 파일을 교체하지 않는다. 성공 시 토큰만 atomic replacement로 `license.json`에 0600 저장한다. 이후 조회도 서명을 다시 검증한다.

라이선스 파일이 **없는** 경우만 기존 30일 체험을 사용한다. unsigned 기존 파일, 손상 파일, 만료 토큰, 누락·손상된 공개키는 active=false와 basic_monitoring만 반환한다. 잘못된 파일을 새 체험으로 바꾸지 않는다. 체험 시작 기록은 최초 create_new로 작성하고 미래·음수·손상 기록과 저장 실패는 활성 체험을 부여하지 않는다.

만료일은 UTC 날짜로 해석하며 해당 날짜까지 유효하다. `expires_at: null`은 발급자가 명시한 영구 라이선스다. 날짜의 잘못된 문자열을 영구 라이선스로 처리하지 않는다. 설치 파일과 시스템 clock을 변경할 수 있는 root 공격자는 체험을 초기화하거나 clock을 되돌릴 수 있다. 현재 구현은 이를 방지하는 trusted clock·외부 원장·장비 attestation이 아니다.

## 오프라인 발급

`tools/license-issuer.py`는 서버와 별도로 보관할 발급 도구다. Python 3.10 이상 및 Ed25519를 지원하는 OpenSSL CLI가 필요하다. 아래 경로는 예시이며 현재 workspace나 운영 서버에 상용 개인키를 생성하지 않았다.

```sh
python3 tools/license-issuer.py generate-key \
  --key-id issuer-2026 \
  --private-key /offline/nabiman/issuer-2026.pem \
  --public-keys /offline/nabiman/license-public-keys.json
```

개인키는 새 파일 0600으로 작성하며 기존 개인키를 덮어쓰지 않는다. 공개키 파일은 원자적으로 교체하고 새 key ID를 추가한다. 발급 작업은 격리된 환경에서 직렬로 실행한다. 파일 기반 도구는 동시 발급자들의 공개키 목록 변경을 조정하는 서비스가 아니다. 개인키의 접근 통제·암호화 보관·백업·HSM 연동·유출 대응은 실제 운영 절차로 추가해야 한다.

발급 claims 예시:

```json
{
  "license_id": "customer-001-contract-001",
  "product": "nabiman",
  "info": {
    "tier": "pro",
    "holder": "Example customer",
    "issued_at": "2026-10-05",
    "expires_at": "2027-10-05",
    "max_servers": 10,
    "features": ["basic_monitoring", "management", "alerts", "backups"]
  }
}
```

```sh
python3 tools/license-issuer.py issue \
  --key-id issuer-2026 \
  --private-key /offline/nabiman/issuer-2026.pem \
  --claims /offline/nabiman/customer-001.json \
  --output /offline/nabiman/customer-001.key
```

중복 JSON 필드·잘못된 필드·feature·한도·날짜를 거부하고 발급 output도 새 0600 파일로 작성한다. 개인키와 발급 토큰을 stdout으로 출력하지 않는다. 미래 발급·만료된 발급 자체는 재현·보관 용도로 가능하지만 서버는 활성화 시 현재 날짜를 검증한다. 개인키는 배포 artifact·컨테이너 image·Git repository·서버 설정에 넣지 않는다.

## 서버 공개키 구성·교체·이전

서버 `NABIMAN_LICENSE_PUBLIC_KEYS_FILE`에 공개키 파일 경로를 설정한다. 기본 경로는 `/etc/nabiman/license-public-keys.json`이다. 형식:

```json
{"keys":[{"key_id":"issuer-2026","public_key":"<32-byte Ed25519 public key as 64 hex characters>"}]}
```

배포는 공개키 파일만 전달한다. 서버 실행 사용자가 읽을 수 있고 신뢰하지 않는 사용자가 변경할 수 없도록 root 소유와 쓰기 권한을 제한한다. 키의 배포·rotation은 versioned 구성 관리 및 별도 감사 절차에 연결해야 한다. 이 코드는 공개키 파일 자체의 출처를 외부 신뢰 anchor로 검증하는 도구는 아니다.

키 교체 때 먼저 새 공개키를 기존 key ID와 함께 배포한다. 다음 새 개인키로 고객 라이선스를 재발급하고 고객의 등록을 확인한다. 마지막 기존 공개키를 제거한다. 기존 키를 제거하면 해당 issuer로 발급한 모든 토큰은 다음 조회부터 무효다. 서버는 공개키를 조회마다 읽으므로 키 교체에는 restart가 필요하지 않다. 오프라인 상태의 개별 고객 라이선스만 취소하는 revocation feed는 아직 없다.

기존 HMAC 토큰과 unsigned 저장 파일은 자동 신뢰·자동 변환하지 않는다. 기존 계약·기능·유효 기간을 확인한 뒤 새 토큰을 발급해야 한다. rollout 전에 기존 파일 목록·고객 영향·발급자 공개키·복구용 이전 release를 준비한다. 잘못된 파일 삭제를 정상 라이선스 이전 절차로 사용하지 않는다.

## Wire format

`NBL1.` + hex(JSON envelope). Envelope 필드는 version=1, key_id, payload(hex JSON claims), signature(hex 64-byte Ed25519 signature)다. 서명 대상 byte sequence:

```text
UTF8("NabiMan license v1") + 0x00 + UTF8(key_id) + 0x00 + decoded_payload_bytes
```

서버는 payload를 재직렬화하여 검증하지 않는다. 전달된 정확한 payload bytes를 검증한다. version은 고정 domain과 서버 version 검사로 분리된다. 알 수 없는 envelope·claims·info 필드는 거부한다. file/token 최대 64KiB, decoded payload 최대 8KiB, 공개키 수 1~64다.

## 남은 상용화 작업

지금까지는 검증 API와 화면·발급 도구의 개선이다. 서비스의 privileged 작업이나 서버 등록 API가 이 라이선스 정보로 차단되는 것은 아니다. 기능·서버 수·조직 단위 사용량의 서버 측 실제 enforcement, 유예 기간·읽기 전용 전환·초과 사용 정책·동시 등록 transaction·고객 안내를 설계하고 시험해야 한다. trial도 현재 기본 feature만 반환한다.

기기·organization binding, 계약·청구·갱신, 개별 라이선스 취소, clock rollback 대응, HSM 발급·백업·복구, 독립 감사 저장, 기존 고객 migration, signed installer 및 artifact provenance는 후속 범위다. 상용 발급 개인키나 production verification root를 아직 구성하지 않았으며 운영 서비스도 변경하지 않았다.
