//! `docs/security/route-access.md`, rendered from the tables, so the
//! document cannot say anything the code does not do.

use super::{all, Class, Member, Route};

const METHODS: [&str; 5] = ["GET", "POST", "PUT", "DELETE", "PATCH"];

fn class_name(class: Class) -> &'static str {
    match class {
        Class::Public => "공개",
        Class::Admin => "관리자 전용",
        Class::Read => "조회",
        Class::Operate => "운영",
        Class::Account => "계정",
    }
}

fn staff(class: Class, operator: bool) -> &'static str {
    match (class, operator) {
        (Class::Public, _) => "(로그인 없이)",
        (Class::Admin, _) => "—",
        (Class::Read, _) | (Class::Operate, false) => "GET",
        (Class::Operate, true) | (Class::Account, _) => "모든 메서드",
    }
}

fn member(m: Member) -> String {
    match m {
        Member::No => "—".into(),
        Member::Only(methods) => methods.join(", "),
        Member::Handler => "본인 것만 (처리기에서 확인)".into(),
        Member::Servers => "서버 규칙 (본인·배정 서버)".into(),
    }
}

fn note(route: &Route) -> String {
    let open: Vec<&str> =
        METHODS.iter().copied().filter(|m| crate::scope_access::every_account(m, route.pattern)).collect();
    if open.is_empty() {
        String::new()
    } else {
        format!("{} 는 모든 계정", open.join(", "))
    }
}

/// What the move to an allowlist kept as it was, but looks wrong (for review;
/// changing it is a decision, not a refactoring).
const REVIEW: &str = concat!(
    "## 검토가 필요해 보이는 권한 (바꾸지 않고 그대로 옮김)\n\n",
    "범위 제한이 없는 운영자 계정 기준입니다. 새로 만드는 운영자는 범위 제한 계정이라 해당하지 않습니다.\n\n",
    "- `POST /api/updates/upgrade`, `/api/updates/upgrade-package`: 운영자가 시스템 전체 업그레이드를 할 수 있습니다.\n",
    "  처리기에 별도 확인이 없고, 반면 `/api/packages/install`·`remove` 는 운영자에게 막혀 있어 서로 맞지 않습니다.\n",
    "- `POST /api/mec/v1/users/{id}/reset-password`, tenant 생성·가져오기·삭제, 노드 cordon·taint 등 MEC 쓰기:\n",
    "  운영자에게 열려 있고, 일부 처리기는 확인 헤더 말고는 역할을 보지 않습니다.\n",
    "- `/api/firewall/add`·`delete`, `/api/ipblock/ban`·`unban`, `/api/services/action`, `/api/sessions/kill`,\n",
    "  `/api/ssl/issue`·`renew`, `/api/license/activate`, `/api/notifications/*`(외부 발송 채널), `/api/alerts/rules*`:\n",
    "  운영자가 바꿀 수 있습니다. 운영 업무로 의도한 것인지 확인이 필요합니다.\n",
    "- 조회자도 읽을 수 있는 것: `/api/logs`(시스템 저널), `/api/cron`(root crontab), `/api/users`(전체 계정 목록),\n",
    "  `/api/notifications/channels`, `/api/database/backups`.\n\n",
);

pub(super) fn render() -> String {
    let mut routes: Vec<&Route> = all().collect();
    routes.sort_by_key(|r| r.pattern);
    let count = |c: Class| routes.iter().filter(|r| r.class == c).count();
    let reach = |f: fn(&Member) -> bool| routes.iter().filter(|r| f(&r.member)).count();
    let mut out = String::new();
    out.push_str(concat!(
        "# API 경로별 접근 권한\n\n",
        "> 이 문서는 `backend/src/route_access/` 의 표에서 만들어집니다. 직접 고치지 말고 표를 고친 뒤\n",
        "> `NABIMAN_WRITE_ROUTE_DOC=1 cargo test route_access` 로 다시 만드세요.\n",
        "> 표와 이 문서가 다르거나, 표에 없는 경로가 등록되어 있으면 테스트가 실패합니다.\n\n",
        "NabiMan 의 API 는 허용 목록 방식입니다. 등록된 모든 경로가 아래 표에 등급과 함께 올라 있고,\n",
        "표에 없는 경로는 최고관리자 말고는 아무도 부를 수 없습니다. 새 기능을 만들면 표에 올리기 전까지\n",
        "운영자·조회자·원격 회원에게 닫혀 있습니다.\n\n",
        "- **최고관리자**(활성 admin)는 이 표로 판단하지 않습니다. 다만 `/api/backup`, `/api/audit`, `/api/apikeys`\n",
        "  는 admin 전용이고, `/api/diagnostics/support-report` 는 admin 의 POST 만 됩니다.\n",
        "- **범위가 정해진 계정**(소속·tenant 로 제한된 운영자·조회자, 새로 만든 계정 모두)은 `scope_access` 의\n",
        "  별도 허용 목록을 따릅니다. 아래 운영자·조회자 열은 범위 제한이 없는 (이전부터 있던) 계정의 권한입니다.\n",
        "- **API 키**(`nbm_…`)는 키에 준 범위의 GET 만 됩니다 (`api_keys`).\n",
        "- 처리기 안에서 한 번 더 확인하는 곳(소유자, 2단계 인증, 최고관리자 확인 등)은 이 표보다 좁을 수 있습니다.\n\n",
        "## 등급\n\n",
        "| 등급 | 운영자(operator) | 조회자(viewer) |\n|---|---|---|\n",
        "| 공개 | 로그인 없이 (처리기가 비밀번호·공유 비밀을 확인) | 같음 |\n",
        "| 관리자 전용 | — | — |\n",
        "| 조회 | GET | GET |\n",
        "| 운영 | 모든 메서드 | GET |\n",
        "| 계정 | 모든 메서드 (자기 로그인·비밀번호·2단계 인증) | 모든 메서드 |\n\n",
        "원격 회원(remote) 열: `—` 는 닫힘, 메서드 목록은 그 메서드만, \"본인 것만\" 은 처리기가 회원 본인의\n",
        "세션·콘솔인지 확인, \"서버 규칙\" 은 `member_servers` 가 경로의 서버가 회원 본인 것이거나 배정된\n",
        "것인지(쓰기는 허용된 경우만) 확인합니다. 관리자가 끈 기능(받아쓰기, 서버 등록, 서버 간 복사)은\n",
        "목록에 있어도 막힙니다.\n\n",
        "## 요약\n\n",
    ));
    out.push_str(&format!(
        "- 등록된 API 경로: {}개\n- 공개 {} · 관리자 전용 {} · 조회 {} · 운영 {} · 계정 {}\n",
        routes.len(),
        count(Class::Public),
        count(Class::Admin),
        count(Class::Read),
        count(Class::Operate),
        count(Class::Account),
    ));
    out.push_str(&format!(
        "- 원격 회원이 닿는 경로: {}개 (메서드 지정 {} · 본인 것만 {} · 서버 규칙 {})\n\n",
        reach(|m| *m != Member::No),
        reach(|m| matches!(m, Member::Only(_))),
        reach(|m| *m == Member::Handler),
        reach(|m| *m == Member::Servers),
    ));
    out.push_str(REVIEW);
    out.push_str("## 경로 표\n\n| 경로 | 등급 | 운영자 | 조회자 | 원격 회원 | 비고 |\n|---|---|---|---|---|---|\n");
    for r in routes {
        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} |\n",
            r.pattern,
            class_name(r.class),
            staff(r.class, true),
            staff(r.class, false),
            member(r.member),
            note(r),
        ));
    }
    out
}
