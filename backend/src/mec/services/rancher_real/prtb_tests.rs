use super::*;
use crate::mec::services::rancher_real::RancherRealConfig;
use std::io::{Read, Write};

async fn response_fixture(body: &str) -> ServiceResult<Vec<ProjectRoleBinding>> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let body = body.to_owned();
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        let mut buffer = [0u8; 1024];
        while !request.windows(4).any(|w| w == b"\r\n\r\n") {
            let count = socket.read(&mut buffer).unwrap();
            assert!(count > 0);
            request.extend_from_slice(&buffer[..count]);
        }
        assert!(request.starts_with(
            b"GET /v3/projectroletemplatebindings?projectId=c%3Ap&limit=1000 HTTP/1.1"
        ));
        write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
    });
    let client = RancherReal::new(RancherRealConfig {
        base_url: format!("http://{address}"),
        token: "fixture-only".into(),
        cluster_id: "c".into(),
        insecure_tls: false,
    })
    .unwrap();
    let result = list(&client, "c:p").await;
    server.join().unwrap();
    result
}

#[tokio::test]
async fn upstream_project_filter_is_encoded_and_response_scope_is_verified() {
    let valid = response_fixture(r#"{"data":[{"id":"prtb-own","projectId":"c:p","userId":"u-1","roleTemplateId":"read-only"}]}"#).await.unwrap();
    assert_eq!(valid[0].project_id, "c:p");
    assert!(response_fixture(r#"{"data":[]}"#).await.unwrap().is_empty());
    for body in [
        r#"{"data":[{"id":"prtb-other","projectId":"c:other"}]}"#,
        r#"{"data":[{"id":"prtb-missing"}]}"#,
        r#"{"data":[{"id":"../users","projectId":"c:p"}]}"#,
    ] {
        let err = response_fixture(body).await.unwrap_err();
        assert_eq!(err.http_status(), 502);
    }
}
async fn pages_fixture(bodies: &[&str]) -> ServiceResult<Vec<ProjectRoleBinding>> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let responses: Vec<_> = bodies
        .iter()
        .map(|body| body.replace("FIXTURE_BASE", &base))
        .collect();
    let server = std::thread::spawn(move || {
        for body in responses {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            let mut socket = loop {
                match listener.accept() {
                    Ok((socket, _)) => break socket,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            std::time::Instant::now() < deadline,
                            "Expected page was never requested"
                        );
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                    Err(error) => panic!("{error}"),
                }
            };
            socket
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let mut buffer = [0; 1024];
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                let count = socket.read(&mut buffer).unwrap();
                assert!(count > 0);
                request.extend_from_slice(&buffer[..count]);
            }
            let request = String::from_utf8(request).unwrap();
            let uri = request.split_whitespace().nth(1).unwrap();
            let url = reqwest::Url::parse(&format!("http://fixture{uri}")).unwrap();
            assert_eq!(url.path(), "/v3/projectroletemplatebindings");
            let projects: Vec<_> = url
                .query_pairs()
                .filter(|(key, _)| key == "projectId")
                .collect();
            assert_eq!(projects.len(), 1);
            assert_eq!(projects[0].1, "c:p");
            write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
        }
    });
    let client = RancherReal::new(RancherRealConfig {
        base_url: base,
        token: "fixture-only".into(),
        cluster_id: "c".into(),
        insecure_tls: false,
    })
    .unwrap();
    let result = list(&client, "c:p").await;
    server.join().unwrap();
    result
}

#[tokio::test]
async fn complete_inventory_follows_scoped_absolute_and_relative_pages() {
    let out = pages_fixture(&[
            r#"{"data":[{"id":"one","projectId":"c:p","userId":"u-1","roleTemplateId":"read-only"}],"pagination":{"partial":true,"total":3,"next":"FIXTURE_BASE/v3/projectroletemplatebindings?marker=two"}}"#,
            r#"{"data":[{"id":"two","projectId":"c:p","userId":"u-2","roleTemplateId":"project-member"}],"pagination":{"partial":true,"total":3,"next":"?marker=three&projectId=c%3Ap"}}"#,
            r#"{"data":[{"id":"three","projectId":"c:p","userId":"u-3","roleTemplateId":"project-owner"}],"pagination":{"partial":true,"total":3,"next":null}}"#,
        ]).await.unwrap();
    assert_eq!(
        out.iter()
            .map(|binding| binding.id.as_str())
            .collect::<Vec<_>>(),
        ["one", "two", "three"]
    );
}

#[tokio::test]
async fn incomplete_or_unsafe_pages_never_become_an_empty_or_partial_success() {
    for body in [
        r#"{"data":[],"pagination":{"partial":true}}"#,
        r#"{"data":[],"pagination":{"partial":true,"total":1}}"#,
        r#"{"data":[],"pagination":{"partial":false,"total":10001}}"#,
        r#"{"error":"not a member list"}"#,
        r#"{"data":[],"pagination":{"next":"https://foreign.example/v3/projectroletemplatebindings"}}"#,
        r#"{"data":[],"pagination":{"next":"FIXTURE_BASE/v3/users"}}"#,
        r#"{"data":[],"pagination":{"next":"?projectId=c:foreign"}}"#,
        r#"{"data":[],"pagination":{"next":"?projectId=c:p&projectId=c:p"}}"#,
        r#"{"data":[],"pagination":{"next":"?marker=x#fragment"}}"#,
        r#"{"data":[{"id":"one","projectId":"c:p","roleTemplateId":"read-only"}]}"#,
    ] {
        assert!(pages_fixture(&[body]).await.is_err());
    }
    assert!(pages_fixture(&[
        r#"{"data":[],"pagination":{"next":"?marker=repeat"}}"#,
        r#"{"data":[],"pagination":{"next":"?marker=repeat"}}"#,
    ])
    .await
    .is_err());
    assert!(pages_fixture(&[
            r#"{"data":[{"id":"one","projectId":"c:p","userId":"u-1","roleTemplateId":"read-only"}],"pagination":{"next":"?marker=two"}}"#,
            r#"{"data":[{"id":"one","projectId":"c:p","userId":"u-1","roleTemplateId":"read-only"}]}"#,
        ]).await.is_err());
}

#[test]
fn pagination_rejects_credentials_and_preserves_project_namespace() {
    let base = reqwest::Url::parse("https://rancher.example/proxy/v3/projectroletemplatebindings")
        .unwrap();
    for next in [
        "https://user:password@rancher.example/proxy/v3/projectroletemplatebindings",
        "http://rancher.example/proxy/v3/projectroletemplatebindings",
        "//foreign.example/proxy/v3/projectroletemplatebindings",
        "../users",
        "?projectId=wrong",
        "?projectId=c:p&projectId=c:p",
        "?marker=x#fragment",
    ] {
        assert!(next_page(&base, &base, next, "c:p").is_err());
    }
    assert!(next_page(&base, &base, &"a".repeat(8193), "c:p").is_err());
    let next = next_page(&base, &base, "?marker=two", "c:p").unwrap();
    assert_eq!(
        next.query_pairs()
            .find(|(key, _)| key == "projectId")
            .unwrap()
            .1,
        "c:p"
    );
}

#[test]
fn rancher_identifiers_cannot_change_paths_or_filters() {
    for id in ["c-m-local:p-abc", "prtb-123", "p_42"] {
        assert!(validate_identifier(id).is_ok());
    }
    for id in [
        "",
        "../users",
        "p?projectId=other",
        "p&limit=-1",
        "p%2fother",
        "p#other",
        "p/other",
        "p\n",
        "한글",
    ] {
        assert!(validate_identifier(id).is_err(), "{id:?}");
    }
    assert!(validate_identifier(&"a".repeat(257)).is_err());
}
