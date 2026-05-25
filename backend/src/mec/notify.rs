use crate::notifications::{send_notification, ChannelStore};

/// Events broadcast from the MEC module. Subject/body is intentionally simple
/// so any downstream channel (email/slack/webhook) renders identically.
pub enum MecNotify<'a> {
    TenantCreated {
        tenant_id: &'a str,
        display_name: &'a str,
        user: &'a str,
    },
    TenantDeleted {
        tenant_id: &'a str,
        user: &'a str,
    },
    TenantCreateFailed {
        tenant_id: &'a str,
        reason: &'a str,
    },
    StarterKitDeployed {
        tenant_id: &'a str,
        user: &'a str,
    },
    GpuModeSwitched {
        node: &'a str,
        from: &'a str,
        to: &'a str,
        user: &'a str,
    },
    NatRuleAdded {
        rule_id: &'a str,
        label: &'a str,
        public_ip: &'a str,
        user: &'a str,
    },
}

impl<'a> MecNotify<'a> {
    pub fn subject(&self) -> String {
        match self {
            Self::TenantCreated { tenant_id, .. } => {
                format!("[NabiMan MEC] 테넌트 생성: {}", tenant_id)
            }
            Self::TenantDeleted { tenant_id, .. } => {
                format!("[NabiMan MEC] 테넌트 삭제: {}", tenant_id)
            }
            Self::TenantCreateFailed { tenant_id, .. } => {
                format!("[NabiMan MEC] 테넌트 생성 실패: {}", tenant_id)
            }
            Self::StarterKitDeployed { tenant_id, .. } => {
                format!("[NabiMan MEC] Starter Kit 배포: {}", tenant_id)
            }
            Self::GpuModeSwitched { node, .. } => {
                format!("[NabiMan MEC] GPU 모드 전환: {}", node)
            }
            Self::NatRuleAdded { rule_id, .. } => {
                format!("[NabiMan MEC] NAT 규칙 추가: {}", rule_id)
            }
        }
    }

    pub fn body(&self) -> String {
        match self {
            Self::TenantCreated {
                tenant_id,
                display_name,
                user,
            } => format!(
                "테넌트 '{}' ({}) 이(가) '{}' 사용자에 의해 생성되었습니다.",
                display_name, tenant_id, user
            ),
            Self::TenantDeleted { tenant_id, user } => format!(
                "테넌트 '{}' 이(가) '{}' 사용자에 의해 삭제되었습니다.",
                tenant_id, user
            ),
            Self::TenantCreateFailed { tenant_id, reason } => format!(
                "테넌트 '{}' 생성 실패. 사유: {}",
                tenant_id, reason
            ),
            Self::StarterKitDeployed { tenant_id, user } => format!(
                "테넌트 '{}' 에 Starter Kit 이 '{}' 사용자에 의해 배포되었습니다.",
                tenant_id, user
            ),
            Self::GpuModeSwitched { node, from, to, user } => format!(
                "노드 '{}' 의 GPU 모드가 {} → {} 로 전환되었습니다 (작업자: {}).",
                node, from, to, user
            ),
            Self::NatRuleAdded {
                rule_id,
                label,
                public_ip,
                user,
            } => format!(
                "NAT 규칙 {} ('{}') 이(가) 공인 IP {} 로 추가되었습니다 (작업자: {}).",
                rule_id, label, public_ip, user
            ),
        }
    }

    pub fn dispatch(&self, channels: &ChannelStore) {
        let list = channels.lock().unwrap().clone();
        if list.is_empty() {
            return;
        }
        let subject = self.subject();
        let body = self.body();
        // Send in background to avoid blocking hot paths.
        std::thread::spawn(move || {
            send_notification(&list, &subject, &body);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subject_format() {
        let e = MecNotify::TenantCreated {
            tenant_id: "t1",
            display_name: "T",
            user: "admin",
        };
        assert!(e.subject().contains("t1"));
    }

    #[test]
    fn body_contains_user() {
        let e = MecNotify::TenantDeleted {
            tenant_id: "t1",
            user: "admin",
        };
        assert!(e.body().contains("admin"));
    }

    #[test]
    fn gpu_body_contains_modes() {
        let e = MecNotify::GpuModeSwitched {
            node: "mec-wn01",
            from: "vm-passthrough",
            to: "time-slicing",
            user: "admin",
        };
        let b = e.body();
        assert!(b.contains("vm-passthrough"));
        assert!(b.contains("time-slicing"));
    }
}
