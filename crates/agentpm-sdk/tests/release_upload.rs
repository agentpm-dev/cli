use agentpm_sdk::{AgentPmClient, client::release_upload::ArtifactUpload};
use serde_json::json;
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::TcpListener,
    sync::{Arc, Mutex},
    time::Duration,
};

#[tokio::test(flavor = "current_thread")]
async fn release_transport_streams_reopens_on_retry_and_keeps_auth_off_storage() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let received = Arc::new(Mutex::new(Vec::new()));
    let captured = received.clone();
    let url = format!("{base}/upload");
    let upload = json!({"artifact_id":"artifact", "target":"any", "put_url":url,
        "headers":{"Content-Type":"application/gzip"}, "expires_at":"tomorrow", "status":"pending", "max_bytes":1_000_000});
    let replies = [
        (200, json!({"release_id":"release", "resumed":true, "expires_at":"tomorrow", "max_bytes":1_000_000, "max_total_bytes":2_000_000, "artifacts":[upload.clone()]}).to_string()),
        (503, "{}".into()), (200, "{}".into()), (200, upload.to_string()),
        (200, json!({"id":"1", "kind":"tool", "name":"fixture", "version":"1.0.0", "url":"/fixture"}).to_string()),
    ];
    let server = std::thread::spawn(move || {
        for (status, body) in replies {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut request = Vec::new();
            let mut buffer = [0_u8; 8192];
            let end = loop {
                let size = stream.read(&mut buffer).unwrap();
                assert_ne!(size, 0);
                request.extend_from_slice(&buffer[..size]);
                if let Some(pos) = request.windows(4).position(|p| p == b"\r\n\r\n") {
                    break pos + 4;
                }
            };
            let headers = String::from_utf8_lossy(&request[..end]).to_ascii_lowercase();
            let length: usize = headers
                .lines()
                .find_map(|line| line.strip_prefix("content-length:"))
                .unwrap()
                .trim()
                .parse()
                .unwrap();
            while request.len() < end + length {
                let size = stream.read(&mut buffer).unwrap();
                assert_ne!(size, 0);
                request.extend_from_slice(&buffer[..size]);
            }
            if headers.starts_with("put ") {
                assert!(!headers.contains("authorization:"));
            } else {
                assert!(headers.contains("authorization: bearer test-token"));
            }
            captured
                .lock()
                .unwrap()
                .push((headers, request[end..].to_vec()));
            write!(stream, "HTTP/1.1 {status} Result\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        }
    });
    let path = std::env::temp_dir().join(format!(
        "agentpm-upload-{}-{}.bin",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let bytes = vec![42_u8; 256 * 1024];
    std::fs::write(&path, &bytes).unwrap();
    let client = AgentPmClient::new(&base).unwrap().with_token("test-token");
    let reservation = client.init_release(&json!({})).await.unwrap();
    assert!(reservation.resumed);
    assert_eq!(reservation.max_total_bytes, Some(2_000_000));
    assert!(
        client
            .upload_release_artifact(&reservation.artifacts[0], &path)
            .await
            .is_err()
    );
    client
        .upload_release_artifact(&reservation.artifacts[0], &path)
        .await
        .unwrap();
    client
        .renew_release_artifact("release", "artifact")
        .await
        .unwrap();
    client.finalize_release("release").await.unwrap();
    server.join().unwrap();
    let requests = received.lock().unwrap();
    assert_eq!(requests[1].1, bytes);
    assert_eq!(requests[2].1, bytes);
    std::fs::remove_file(path).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn server_limit_rejects_a_sparse_file_before_upload_or_buffering() {
    let path =
        std::env::temp_dir().join(format!("agentpm-upload-limit-{}.bin", std::process::id()));
    let file = std::fs::File::create(&path).unwrap();
    // Windows does not necessarily make set_len sparse; keep that fixture small.
    let length = if cfg!(windows) {
        16 * 1024 * 1024
    } else {
        4 * 1024 * 1024 * 1024
    };
    file.set_len(length).unwrap();
    drop(file);
    let instruction = ArtifactUpload {
        artifact_id: "artifact".into(),
        target: "any".into(),
        put_url: "http://127.0.0.1:1/must-not-connect".into(),
        headers: HashMap::new(),
        expires_at: "tomorrow".into(),
        status: "pending".into(),
        max_bytes: 1024,
    };
    let error = AgentPmClient::new("http://127.0.0.1:1")
        .unwrap()
        .upload_release_artifact(&instruction, &path)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("server policy"));
    std::fs::remove_file(path).unwrap();
}
