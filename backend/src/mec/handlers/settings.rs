use super::util::ok_response;
use actix_web::HttpResponse;
use serde::Serialize;

#[derive(Serialize)]
pub struct SettingsSnapshot {
    pub mode: String,
    pub read_only: bool,
    pub skip_confirm: bool,
    pub kubernetes: KubeSettings,
    pub rancher: EndpointSettings,
    pub axgate: AxgateSettings,
    pub harbor: EndpointSettings,
    pub systemd_dropin_sample: String,
}

#[derive(Serialize)]
pub struct KubeSettings {
    pub kubeconfig_path: Option<String>,
    pub namespace: String,
    pub configured: bool,
}

#[derive(Serialize)]
pub struct EndpointSettings {
    pub url: Option<String>,
    pub username_configured: bool,
    pub secret_configured: bool,
    pub insecure_tls: bool,
    pub configured: bool,
}

#[derive(Serialize)]
pub struct AxgateSettings {
    pub host: Option<String>,
    pub port: u16,
    pub username_configured: bool,
    pub password_configured: bool,
    pub configured: bool,
}

pub async fn snapshot() -> HttpResponse {
    let snap = SettingsSnapshot {
        mode: env("NABIMAN_MEC_MODE").unwrap_or_else(|| "auto".into()),
        read_only: crate::mec::safety::is_read_only(),
        skip_confirm: env("NABIMAN_MEC_SKIP_CONFIRM")
            .map(|v| v == "1" || v == "true")
            .unwrap_or(false),
        kubernetes: kube_settings(),
        rancher: rancher_settings(),
        axgate: axgate_settings(),
        harbor: harbor_settings(),
        systemd_dropin_sample: DROPIN_SAMPLE.to_string(),
    };
    ok_response(snap)
}

fn env(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.is_empty())
}

fn kube_settings() -> KubeSettings {
    let path = env("NABIMAN_MEC_KUBECONFIG");
    KubeSettings {
        configured: path.is_some() || in_cluster(),
        kubeconfig_path: path,
        namespace: env("NABIMAN_MEC_NAMESPACE").unwrap_or_else(|| "default".into()),
    }
}

fn in_cluster() -> bool {
    std::path::Path::new("/var/run/secrets/kubernetes.io/serviceaccount/token").exists()
}

fn rancher_settings() -> EndpointSettings {
    let url = env("NABIMAN_MEC_RANCHER_URL");
    let token = env("NABIMAN_MEC_RANCHER_TOKEN");
    EndpointSettings {
        configured: url.is_some() && token.is_some(),
        url,
        username_configured: false,
        secret_configured: token.is_some(),
        insecure_tls: env("NABIMAN_MEC_RANCHER_INSECURE")
            .map(|v| v == "1" || v == "true")
            .unwrap_or(false),
    }
}

fn axgate_settings() -> AxgateSettings {
    let host = env("NABIMAN_MEC_AXGATE_HOST");
    let user = env("NABIMAN_MEC_AXGATE_USER");
    let pw = env("NABIMAN_MEC_AXGATE_PASSWORD");
    AxgateSettings {
        configured: host.is_some() && user.is_some() && pw.is_some(),
        host,
        port: env("NABIMAN_MEC_AXGATE_PORT")
            .and_then(|v| v.parse().ok())
            .unwrap_or(2222),
        username_configured: user.is_some(),
        password_configured: pw.is_some(),
    }
}

fn harbor_settings() -> EndpointSettings {
    let url = env("NABIMAN_MEC_HARBOR_URL");
    let user = env("NABIMAN_MEC_HARBOR_USER");
    let pw = env("NABIMAN_MEC_HARBOR_PASSWORD");
    EndpointSettings {
        configured: url.is_some() && user.is_some() && pw.is_some(),
        url,
        username_configured: user.is_some(),
        secret_configured: pw.is_some(),
        insecure_tls: env("NABIMAN_MEC_HARBOR_INSECURE")
            .map(|v| v == "1" || v == "true")
            .unwrap_or(false),
    }
}

const DROPIN_SAMPLE: &str = r#"# /etc/systemd/system/nabiman.service.d/mec.conf
[Service]
# 안전: 첫 연결 시 true 로 두고 Health/조회만 확인 후 false 로 전환하세요.
Environment=NABIMAN_MEC_READ_ONLY=true
Environment=NABIMAN_MEC_MODE=real
Environment=NABIMAN_MEC_KUBECONFIG=/etc/nabiman/kubeconfig
Environment=NABIMAN_MEC_RANCHER_URL=https://rancher.jcia.mec.local
Environment=NABIMAN_MEC_RANCHER_TOKEN=token-xxxxx:xxxxxxxxxxxxxxxx
Environment=NABIMAN_MEC_RANCHER_CLUSTER=local
Environment=NABIMAN_MEC_RANCHER_INSECURE=true
Environment=NABIMAN_MEC_AXGATE_HOST=121.147.13.228
Environment=NABIMAN_MEC_AXGATE_PORT=2222
Environment=NABIMAN_MEC_AXGATE_USER=admin
Environment=NABIMAN_MEC_AXGATE_PASSWORD=***
Environment=NABIMAN_MEC_HARBOR_URL=https://harbor.jcia.mec.local
Environment=NABIMAN_MEC_HARBOR_USER=admin
Environment=NABIMAN_MEC_HARBOR_PASSWORD=***
Environment=NABIMAN_MEC_HARBOR_INSECURE=true

# 적용:
# sudo chmod 600 /etc/systemd/system/nabiman.service.d/mec.conf
# sudo systemctl daemon-reload
# sudo systemctl restart nabiman
"#;
