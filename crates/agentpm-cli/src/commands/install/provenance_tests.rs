use super::InstallArgs;
use axum::{
    Json, Router,
    routing::{get, post},
};
use clap::Parser;
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[test]
fn install_parses_both_cryptographic_requirement_flags() {
    let args = InstallArgs::try_parse_from([
        "install",
        "@fixture/example",
        "--require-signature",
        "--require-attestation",
    ])
    .unwrap();
    assert!(args.require_signature && args.require_attestation);
}

#[tokio::test]
async fn counts_and_boolean_attestation_cannot_complete_an_explicitly_verified_install() {
    for attestation in [false, true] {
        let touched = Arc::new(AtomicBool::new(false));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let download_url = format!("{origin}/artifact");
        let downloads = touched.clone();
        let finalizes = touched.clone();
        let app=Router::new()
            .route("/v1/tools/install/resolve",post(|| async { Json(json!({"items":[{"kind":"tool","name":"@fixture/example","version":"1.2.0","integrity":"a".repeat(64)}]})) }))
            .route("/v1/tools/install/init",post(move || async move { Json(json!({"session_id":"fixture","expires_at":"2030-01-01T00:00:00Z","artifacts":[{"kind":"tool","name":"@fixture/example","version":"1.2.0","integrity":"a".repeat(64),"presigned_url":download_url,"signing":{"mode":"optional","min_author_signatures":1,"author_signatures_present":1,"registry_attested":true}}]})) }))
            .route("/artifact",get(move || async move { downloads.store(true,Ordering::SeqCst); "must not download" }))
            .route("/v1/tools/install/finalize",post(move || async move { finalizes.store(true,Ordering::SeqCst); "must not finalize" }));
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let root = std::env::temp_dir().join(format!(
            "agentpm-m14c-cli-{}-{}",
            std::process::id(),
            attestation
        ));
        std::fs::create_dir_all(&root).unwrap();
        let args = InstallArgs {
            manifest: root.join("agent.json").to_string_lossy().into_owned(),
            spec: Some("@fixture/example@1.2.0".into()),
            frozen: false,
            refresh: false,
            update_range: false,
            quiet: true,
            require_signature: !attestation,
            require_attestation: attestation,
            token: None,
        };
        assert!(args.run(origin).await.is_err());
        assert!(!touched.load(Ordering::SeqCst));
        assert!(!root.join("agent.lock").exists());
        assert!(!root.join(".agentpm").exists());
        server.abort();
        std::fs::remove_dir_all(root).unwrap();
    }
}
