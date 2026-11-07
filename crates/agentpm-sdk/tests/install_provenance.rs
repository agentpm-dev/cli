use agentpm_sdk::{AgentPmClient, models::install::ResolveResponse};
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
