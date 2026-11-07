use agentpm_sdk::{
    AgentPmClient,
    provenance::{SignedKeySet, TrustRoot},
};
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use ed25519_dalek::{Signer, SigningKey};
use serde_json::Value;
use std::{
    io::{Read, Write},
    net::TcpListener,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[tokio::test]
async fn only_root_verified_keysets_enter_cache_and_cached_bytes_are_reverified() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let fixture: Value = serde_json::from_str(include_str!(
        "../contracts/registry-provenance-v1.fixtures.json"
    ))
    .unwrap();
    let mut signed: SignedKeySet = serde_json::from_value(fixture["keyset"].clone()).unwrap();
    signed.statement.registry = origin.clone();
    signed.signature_b64 = B64.encode(
        SigningKey::from_bytes(&[42; 32])
            .sign(&signed.statement.canonical_bytes().unwrap())
            .to_bytes(),
    );
    let valid = serde_json::to_vec(&signed).unwrap();
    signed.signature_b64 = B64.encode([0; 64]);
    let forged = serde_json::to_vec(&signed).unwrap();
    let expected = valid.clone();
    let server = std::thread::spawn(move || {
        for body in [valid, forged] {
            let (mut socket, _) = listener.accept().unwrap();
            socket.set_nonblocking(false).unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                socket.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            assert!(
                !String::from_utf8_lossy(&request)
                    .to_ascii_lowercase()
                    .contains("authorization:")
            );
            write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).unwrap();
            socket.write_all(&body).unwrap();
        }
    });
    let mut roots: Vec<TrustRoot> = serde_json::from_value(fixture["roots"].clone()).unwrap();
    roots[0].registry = origin.clone();
    let now = chrono::DateTime::parse_from_rfc3339(fixture["now"].as_str().unwrap())
        .unwrap()
        .timestamp();
    let path = std::env::temp_dir().join(format!(
        "agentpm-m14c-cache-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let client = AgentPmClient::new(origin)
        .unwrap()
        .with_token("apm_not-for-public-key-endpoint");
    client
        .verified_registry_keys(&roots, Some(&path), now)
        .await
        .unwrap();
    let cache = std::fs::read_dir(&path)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(std::fs::read(&cache).unwrap(), expected);
    assert!(
        client
            .verified_registry_keys(&roots, Some(&path), now)
            .await
            .is_err()
    );
    assert_eq!(std::fs::read(&cache).unwrap(), expected); // Bad 200 cannot overwrite/fall back.
    server.join().unwrap();
    client
        .verified_registry_keys(&roots, Some(&path), now)
        .await
        .unwrap(); // Network down; valid cached proof.
    assert!(
        client
            .verified_registry_keys(&roots, Some(&path), now + 91 * 86400)
            .await
            .is_err()
    );
    let mut changed: Value = serde_json::from_slice(&expected).unwrap();
    changed["signatureB64"] = B64.encode([0; 64]).into();
    std::fs::write(&cache, serde_json::to_vec(&changed).unwrap()).unwrap();
    assert!(
        client
            .verified_registry_keys(&roots, Some(&path), now)
            .await
            .is_err()
    );
    std::fs::remove_dir_all(path).unwrap();
}
