# NabiMan - MEC Management 모듈 설계 문서

> **목적**: JCIA 5G MEC 테스트베드의 테넌트/노드/GPU/네트워크/방화벽 운영을 NabiMan 대시보드에 통합하여, 현재 수작업/스크립트로 이루어지는 관리 작업을 UI 기반으로 자동화.

## 문서 구성

| 문서 | 내용 |
|------|------|
| [01-overview.md](./01-overview.md) | 배경, 목적, 적용 범위, 성공 기준 |
| [02-architecture.md](./02-architecture.md) | 전체 아키텍처, 컴포넌트 다이어그램, 데이터 흐름 |
| [03-menu-ui.md](./03-menu-ui.md) | 메뉴 구조, 화면 레이아웃, UX 플로우 |
| [04-api-design.md](./04-api-design.md) | REST API 명세, 엔드포인트 목록 |
| [05-data-models.md](./05-data-models.md) | 내부 데이터 모델 (Rust struct / DB schema) |
| [06-roadmap.md](./06-roadmap.md) | 구현 단계, MVP 범위, 일정 |
| [07-integration-prerequisites.md](./07-integration-prerequisites.md) | NabiMan 서버 ↔ MEC 연동 전 네트워크/자격증명 준비사항 |
| [07-runbook.md](./07-runbook.md) | 운영자 런북 — 첫 연결, Read-only 검증, 롤백, 장애 대응 |
| [08-handoff-to-server.md](./08-handoff-to-server.md) | MEC 운영팀 → NabiMan 서버팀 인계 (260420) |

## 한눈에 보는 설계 요약

### 핵심 기능 (Phase 1 MVP)

| 기능 | 설명 |
|------|------|
| **테넌트 관리** | 원클릭 생성/삭제 (NS + Rancher Project + User + Quota + Taint + 스타터킷) |
| **노드 관리** | 노드 현황, Taint/Label, GPU 모드 전환 (VFIO ↔ Time-Slicing) |
| **GPU 자원 현황** | 실시간 GPU Slots 할당/여유, 테넌트별 사용량 |
| **LB IP 관리** | MetalLB Pool 현황, 서비스별 IP 매핑 |
| **AXGATE 방화벽** | NAT 규칙 조회/추가/삭제 (proxy-arp, DNAT, 보안정책) |
| **접속 가이드 자동 생성** | 기업별 접속 정보를 docx/pdf로 즉시 출력 |

### 기술 스택 (NabiMan 기존 스택 준수)

| 계층 | 기술 |
|------|------|
| Frontend | React + TypeScript + (기존 UI 라이브러리) |
| Backend | **Rust + Actix-web + kube-rs + ssh2** |
| Config | 단일 바이너리 배포 (기존 방식 유지) |
| 외부 연동 | Kubernetes API, Rancher API, AXGATE CLI (SSH), Harbor API |

### 주요 외부 시스템 연동

```
┌──────────────────────────┐
│   NabiMan (React UI)     │
└────────┬─────────────────┘
         │ HTTP/REST
┌────────▼─────────────────┐
│ NabiMan Backend (Rust)   │
│   ├─ kube-rs (K8s API)   │────► Kubernetes API Server (172.20.26.239:6443)
│   ├─ reqwest (Rancher)   │────► Rancher API (rancher.jcia.mec.local)
│   ├─ ssh2 (AXGATE CLI)   │────► AXGATE 방화벽 (121.147.13.228:2222)
│   └─ reqwest (Harbor)    │────► Harbor API (172.20.26.234)
└──────────────────────────┘
```

## 구현 우선순위

1. **Phase 1 (MVP, 2주)**: 테넌트 CRUD + GPU/노드 현황 대시보드
2. **Phase 2 (2주)**: LB IP/방화벽 NAT 통합 관리
3. **Phase 3 (2주)**: 접속 가이드 자동 생성 + Harbor 연동
4. **Phase 4 (선택)**: 실시간 모니터링(Prometheus) 임베드

## 참고 자료

- 기존 operator-kit 스크립트: `/root/mec-project/operator-kit/scripts/`
- NabiMan 기존 구조: `I:\backend`, `I:\frontend`
- 접속 가이드 (5개 기업): `C:\Users\master\5G MEC\접속가이드_*.docx`
- 자원 할당 계획: `C:\Users\master\5G MEC\1차_실증기업_자원할당_계획_260401.md`
