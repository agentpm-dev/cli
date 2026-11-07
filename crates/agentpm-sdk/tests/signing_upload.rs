use agentpm_sdk::{AgentPmClient, signing::ReleaseAuthorSignature};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    thread::JoinHandle,
    time::Duration,
};

type CapturedRequests = Vec<(String, Vec<u8>)>;

fn server(
    final_status: u16,
    final_response: Value,
    legacy: bool,
) -> (String, JoinHandle<CapturedRequests>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let replies = if legacy {
        vec![
            (
                200,
                json!({"upload_id":"fixture","put_url":format!("{base}/storage"),"headers":{},"tmp_key":"uploads/test","final_key":"tools/test","expires_in":900}),
            ),
            (200, json!({})),
            (final_status, final_response),
        ]
    } else {
        vec![(final_status, final_response)]
    };
    let handle = std::thread::spawn(move || {
        let mut requests = Vec::new();
        for (status, response) in replies {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut raw = Vec::new();
            let mut buffer = [0; 8192];
            let end = loop {
                let count = stream.read(&mut buffer).unwrap();
                assert!(count > 0);
                raw.extend_from_slice(&buffer[..count]);
                if let Some(index) = raw.windows(4).position(|p| p == b"\r\n\r\n") {
                    break index + 4;
                }
            };
            let headers = String::from_utf8_lossy(&raw[..end]).to_ascii_lowercase();
            let length: usize = headers
                .lines()
                .find_map(|s| s.strip_prefix("content-length:"))
                .unwrap()
                .trim()
                .parse()
                .unwrap();
            while raw.len() < end + length {
                let count = stream.read(&mut buffer).unwrap();
                assert!(count > 0);
                raw.extend_from_slice(&buffer[..count]);
            }
            if headers.starts_with("put ") {
                assert!(!headers.contains("authorization:"));
            } else {
                assert!(headers.contains("authorization: bearer fixture-token"));
            }
            requests.push((headers, raw[end..].to_vec()));
            let body = response.to_string();
            write!(stream,"HTTP/1.1 {status} Result\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        }
        requests
    });
    (base, handle)
}

fn receipt() -> Value {
    json!({"id":"1","name":"example","version":"1.0.0","url":"/example"})
}

fn file() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "agentpm-signing-upload-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, b"fixture archive bytes").unwrap();
    path
}

#[tokio::test(flavor = "current_thread")]
async fn explicit_legacy_signing_never_succeeds_without_server_acceptance() {
    for acknowledgement in [None, Some(0), Some(1)] {
        let mut response = receipt();
        if let Some(count) = acknowledgement {
            response["author_signatures_accepted"] = json!(count);
        }
        let (base, server) = server(200, response, true);
        let path = file();
        let extra = json!({"require_signature":true,"author_signatures":[{"statement_json":{"type":"agentpm.package.signature.v1"}}]});
        let result = AgentPmClient::new(&base)
            .unwrap()
            .with_token("fixture-token")
            .publish_package_from_path(&json!({}), &path, "fixture.tar.gz", Some(extra.clone()))
            .await;
        assert_eq!(result.is_ok(), acknowledgement == Some(1));
        if let Err(error) = result {
            assert!(error.to_string().contains("did not confirm"));
        }
        let requests = server.join().unwrap();
        let final_body: Value = serde_json::from_slice(&requests.last().unwrap().1).unwrap();
        assert_eq!(final_body["require_signature"], true);
        assert_eq!(final_body["author_signatures"], extra["author_signatures"]);
        std::fs::remove_file(path).unwrap();
    }
    let response = json!({"code":"unregistered_signer","message":"Register this public signing key in the publish namespace"});
    let (base, server) = server(422, response, true);
    let path = file();
    let result = AgentPmClient::new(&base)
        .unwrap()
        .with_token("fixture-token")
        .publish_package_from_path(
            &json!({}),
            &path,
            "fixture.tar.gz",
            Some(json!({"require_signature":true})),
        )
        .await;
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("unregistered_signer") && error.contains("Register this public signing key")
    );
    server.join().unwrap();
    std::fs::remove_file(path).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn signed_release_finalize_carries_typed_release_statement() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../contracts/release-signing-v1.fixtures.json"
    ))
    .unwrap();
    let envelope:ReleaseAuthorSignature=serde_json::from_value(json!({"algo":"ed25519","public_key_b64":fixture["publicKeyB64"],"signature_b64":fixture["vectors"][0]["signatureB64"],"statement_json":fixture["vectors"][0]["statement"]})).unwrap();
    let mut response = receipt();
    response["author_signatures_accepted"] = json!(1);
    let (base, server) = server(200, response, false);
    let client = AgentPmClient::new(&base)
        .unwrap()
        .with_token("fixture-token");
    client
        .finalize_signed_release("fixture", &[envelope])
        .await
        .unwrap();
    let requests = server.join().unwrap();
    assert!(
        requests[0]
            .0
            .starts_with("post /v1/tools/publish/releases/fixture/finalize ")
    );
    let body: Value = serde_json::from_slice(&requests[0].1).unwrap();
    assert_eq!(body["require_signature"], true);
    assert_eq!(
        body["author_signatures"][0]["statement_json"],
        fixture["vectors"][0]["statement"]
    );
}
