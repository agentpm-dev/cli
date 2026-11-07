use agentpm_sdk::{
    AgentPmClient,
    models::install::{InstallInitResponse, ResolveResponse},
};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::TcpListener,
    time::Duration,
};

#[tokio::test]
async fn shared_install_client_rejects_plan_substitution_and_required_signature_downgrade() {
    for failure in ["identity", "integrity", "missing", "required"] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let mut artifact = json!({"kind":"tool","name":"@fixture/example","version":"1.2.0","integrity":"a".repeat(64),"presigned_url":"https://storage.fixture.invalid/archive"});
        match failure {
            "identity" => artifact["name"] = json!("@fixture/other"),
            "integrity" => artifact["integrity"] = json!("b".repeat(64)),
            "required" => {
                artifact["signing"] = json!({"mode":"required","min_author_signatures":1,"author_signatures_present":1,"registry_attested":true})
            }
            _ => {}
        }
        let body=serde_json::to_vec(&json!({"session_id":"fixture","expires_at":"2030-01-01T00:00:00Z","artifacts":if failure=="missing" {vec![]} else {vec![artifact]}})).unwrap();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket.set_nonblocking(false).unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                socket.read_exact(&mut byte).unwrap();
                header.push(byte[0]);
            }
            let header = String::from_utf8(header).unwrap();
            let length: usize = header
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|v| v.trim().parse().unwrap())
                })
                .unwrap();
            let mut request = vec![0; length];
            socket.read_exact(&mut request).unwrap();
            let request: Value = serde_json::from_slice(&request).unwrap();
            assert_eq!(request["items"][0]["name"], "@fixture/example");
            write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).unwrap();
            socket.write_all(&body).unwrap();
        });
        let plan:ResolveResponse=serde_json::from_value(json!({"items":[{"kind":"tool","name":"@fixture/example","version":"1.2.0","integrity":"a".repeat(64)}]})).unwrap();
        let error = AgentPmClient::new(origin)
            .unwrap()
            .install_init(&plan)
            .await
            .unwrap_err();
        let message = error.to_string();
        if failure == "required" {
            assert!(message.contains("cryptographic provenance"), "{message}");
        } else {
            assert!(
                message.contains("package") || message.contains("integrity"),
                "{message}"
            );
        }
        server.join().unwrap();
    }
}

#[tokio::test]
async fn renewal_rejects_changed_session_duplicate_packages_and_changed_digest() {
    for mutation in ["session", "duplicate", "digest"] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let artifact = |name| json!({"kind":"tool","name":name,"version":"1.0.0","integrity":"a".repeat(64),"presigned_url":"https://storage.invalid/old"});
        let original: InstallInitResponse = serde_json::from_value(json!({"session_id":"original","expires_at":"2030-01-01T00:00:00Z", "artifacts":[artifact("@fixture/a"), artifact("@fixture/b")]})).unwrap();
        let mut changed = serde_json::to_value(&original).unwrap();
        match mutation {
            "session" => changed["session_id"] = json!("other"),
            "duplicate" => changed["artifacts"][1] = changed["artifacts"][0].clone(),
            _ => changed["artifacts"][0]["integrity"] = json!("f".repeat(64)),
        }
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket.set_nonblocking(false).unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                socket.read_exact(&mut byte).unwrap();
                header.push(byte[0]);
            }
            let header = String::from_utf8(header).unwrap();
            assert!(header.starts_with("POST /v1/tools/install/renew"));
            let length: usize = header
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|v| v.trim().parse().unwrap())
                })
                .unwrap();
            let mut body = vec![0; length];
            socket.read_exact(&mut body).unwrap();
            let body = changed.to_string();
            write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        });
        let error = AgentPmClient::new(origin)
            .unwrap()
            .install_renew(&original)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("renewal"));
        server.join().unwrap();
    }
}
